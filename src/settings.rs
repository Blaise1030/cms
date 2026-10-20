//! Per-project settings from `orso.json` at the repo root.
//!
//! The file and every field are optional. Missing fields fall back to the
//! defaults, empty strings mean "unset", and validation reports per-field
//! errors so the UI can point at the broken field.

use crate::content::catalog;
use crate::content::lifecycle::{Journal, Undo};
use crate::content::mutation::{
    emit_checkpoint, lock_for, project_root, read_current, require_content_change, resolve, stale,
    write_atomic,
};
use crate::{error::ApiError, git, state::AppState};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub const SETTINGS_FILE: &str = "orso.json";
pub const HOST: &str = "127.0.0.1";

const DEFAULT_INSTALL: &str = "npm install --no-audit --no-fund --no-progress";
const DEFAULT_DEV: &str = "npm run dev -- --port {port} --host {host} --ignore-lock";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSettings {
    pub root: String,
    pub install_dir: String,
    pub install: String,
    pub dev: String,
}

impl ProjectSettings {
    pub fn defaults() -> Self {
        Self {
            root: ".".to_string(),
            install_dir: ".".to_string(),
            install: DEFAULT_INSTALL.to_string(),
            dev: DEFAULT_DEV.to_string(),
        }
    }

    pub fn site_dir(&self, project: &Path) -> PathBuf {
        join_rel(project, &self.root)
    }

    pub fn install_path(&self, project: &Path) -> PathBuf {
        join_rel(project, &self.install_dir)
    }

    pub fn dev_command(&self, port: u16) -> String {
        self.dev
            .replace("{port}", &port.to_string())
            .replace("{host}", HOST)
    }

    pub fn site_prefix(&self) -> String {
        if self.root == "." {
            String::new()
        } else {
            format!("{}/", self.root)
        }
    }
}

pub type FieldErrors = BTreeMap<String, String>;

fn join_rel(project: &Path, rel: &str) -> PathBuf {
    if rel == "." {
        project.to_path_buf()
    } else {
        project.join(rel)
    }
}

/// Normalize a relative path: `"."` or `a/b`, no `./`, no trailing `/`.
/// `.` and empty components are dropped; `..` pops a component when it can.
fn normalize_rel(raw: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in raw.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|p| *p != "..") {
                    parts.pop();
                } else {
                    parts.push(part);
                }
            }
            _ => parts.push(part),
        }
    }
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

fn string_field(raw: &Map<String, Value>, key: &str, errors: &mut FieldErrors) -> Option<String> {
    match raw.get(key) {
        None => None,
        Some(Value::String(s)) if s.is_empty() => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(_) => {
            errors.insert(key.to_string(), "must be a string".to_string());
            None
        }
    }
}

fn has_astro_config(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry
            .file_name()
            .to_string_lossy()
            .starts_with("astro.config.")
            && entry.path().is_file()
    })
}

/// Validate a directory field (`root`/`installDir`). Returns the normalized
/// value, or records an error under `key` and returns `None`.
fn validate_dir(
    project: &Path,
    key: &str,
    value: &str,
    explicit: bool,
    require_astro: bool,
    errors: &mut FieldErrors,
) -> Option<String> {
    if Path::new(value).is_absolute() || value.starts_with('/') {
        errors.insert(key.to_string(), "must be a relative path".to_string());
        return None;
    }
    let normalized = normalize_rel(value);
    let candidate = join_rel(project, &normalized);
    let canonical_project = match project.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            errors.insert(key.to_string(), format!("project does not exist: {e}"));
            return None;
        }
    };
    let canonical = match candidate.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            errors.insert(key.to_string(), "does not exist".to_string());
            return None;
        }
    };
    if !canonical.starts_with(&canonical_project) {
        errors.insert(key.to_string(), "must stay inside the project".to_string());
        return None;
    }
    if !canonical.is_dir() {
        errors.insert(key.to_string(), "must be a directory".to_string());
        return None;
    }
    if require_astro && explicit && !has_astro_config(&canonical) {
        errors.insert(
            key.to_string(),
            "must contain an astro.config.* file".to_string(),
        );
        return None;
    }
    Some(normalized)
}

pub fn validate(project: &Path, raw: &Map<String, Value>) -> Result<ProjectSettings, FieldErrors> {
    let defaults = ProjectSettings::defaults();
    let mut errors = FieldErrors::new();

    let root_raw = string_field(raw, "root", &mut errors);
    let root_explicit = root_raw.is_some();
    let root = match &root_raw {
        Some(value) => validate_dir(project, "root", value, root_explicit, true, &mut errors),
        None if errors.contains_key("root") => None,
        None => validate_dir(project, "root", &defaults.root, false, true, &mut errors),
    };

    let install_dir_raw = string_field(raw, "installDir", &mut errors);
    let install_dir = match &install_dir_raw {
        Some(value) => validate_dir(project, "installDir", value, true, false, &mut errors),
        None if errors.contains_key("installDir") => None,
        None => validate_dir(
            project,
            "installDir",
            &defaults.install_dir,
            false,
            false,
            &mut errors,
        ),
    };

    let install = match string_field(raw, "install", &mut errors) {
        Some(value) => Some(value),
        None if errors.contains_key("install") => None,
        None => Some(defaults.install.clone()),
    };

    let dev = match string_field(raw, "dev", &mut errors) {
        Some(value) => {
            if value.contains("{port}") {
                Some(value)
            } else {
                errors.insert("dev".to_string(), "must contain {port}".to_string());
                None
            }
        }
        None if errors.contains_key("dev") => None,
        None => Some(defaults.dev.clone()),
    };

    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(ProjectSettings {
        root: root.unwrap_or(defaults.root),
        install_dir: install_dir.unwrap_or(defaults.install_dir),
        install: install.unwrap_or(defaults.install),
        dev: dev.unwrap_or(defaults.dev),
    })
}

pub fn load(project: &Path) -> Result<ProjectSettings, FieldErrors> {
    let path = project.join(SETTINGS_FILE);
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ProjectSettings::defaults());
        }
        Err(e) => {
            let mut errors = FieldErrors::new();
            errors.insert("_file".to_string(), format!("could not be read: {e}"));
            return Err(errors);
        }
    };
    let value: Value = match serde_json::from_str(&content) {
        Ok(value) => value,
        Err(e) => {
            let mut errors = FieldErrors::new();
            errors.insert("_file".to_string(), format!("invalid JSON: {e}"));
            return Err(errors);
        }
    };
    match value {
        Value::Object(map) => validate(project, &map),
        _ => {
            let mut errors = FieldErrors::new();
            errors.insert("_file".to_string(), "must be a JSON object".to_string());
            Err(errors)
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDoc {
    pub exists: bool,
    pub settings: Map<String, Value>,
    pub defaults: ProjectSettings,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    pub branch: String,
    pub errors: FieldErrors,
}

#[derive(Deserialize)]
pub struct SaveSettings {
    pub project: String,
    pub branch: String,
    pub hash: Option<String>,
    pub settings: Map<String, Value>,
}

pub async fn read(state: &AppState, project: &str) -> Result<SettingsDoc, ApiError> {
    let root = project_root(state, project).await?;
    let path = resolve(&root, SETTINGS_FILE)?;
    let branch = catalog::current_branch(&root).await?;
    if std::fs::symlink_metadata(&path).is_err() {
        return Ok(SettingsDoc {
            exists: false,
            settings: Map::new(),
            defaults: ProjectSettings::defaults(),
            hash: None,
            branch,
            errors: FieldErrors::new(),
        });
    }
    let source = read_current(&path)?;
    let (settings, errors) = match serde_json::from_str::<Value>(&source) {
        Ok(Value::Object(map)) => {
            let errors = validate(&root, &map).err().unwrap_or_default();
            (map, errors)
        }
        Ok(_) => {
            let mut errors = FieldErrors::new();
            errors.insert("_file".to_string(), "must be a JSON object".to_string());
            (Map::new(), errors)
        }
        Err(e) => {
            let mut errors = FieldErrors::new();
            errors.insert("_file".to_string(), format!("invalid JSON: {e}"));
            (Map::new(), errors)
        }
    };
    Ok(SettingsDoc {
        exists: true,
        settings,
        defaults: ProjectSettings::defaults(),
        hash: Some(catalog::hash(&source)),
        branch,
        errors,
    })
}

pub async fn save(state: &AppState, req: SaveSettings) -> Result<SettingsDoc, ApiError> {
    let root = project_root(state, &req.project).await?;
    let lock = lock_for(&root);
    let _guard = lock.lock().await;
    require_content_change(&root, &req.branch).await?;

    // Empty-string known fields mean "unset": drop them before validating/writing.
    let cleaned: Map<String, Value> = req
        .settings
        .iter()
        .filter(|(k, v)| {
            !(matches!(k.as_str(), "root" | "installDir" | "install" | "dev")
                && matches!(v, Value::String(s) if s.is_empty()))
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if let Err(errors) = validate(&root, &cleaned) {
        let details: Vec<Value> = errors
            .iter()
            .map(|(field, message)| json!({ "path": [field], "message": message }))
            .collect();
        return Err(ApiError::badreq(describe(&errors))
            .with_reason("invalid_settings")
            .with_details(json!(details)));
    }
    let source = format!(
        "{}\n",
        serde_json::to_string_pretty(&Value::Object(cleaned))
            .map_err(|e| ApiError::badreq(e.to_string()))?
    );

    let path = resolve(&root, SETTINGS_FILE)?;
    let (old, mode) = match (std::fs::symlink_metadata(&path), &req.hash) {
        (Err(_), None) => (None, 0o644),
        (Ok(meta), Some(hash)) if meta.is_file() => {
            let old = read_current(&path)?;
            if &catalog::hash(&old) != hash {
                return Err(stale());
            }
            // Unlike DESIGN.md, an unparseable file may be repaired here:
            // the matching hash proves the writer saw its current bytes.
            (Some(old), meta.permissions().mode() & 0o7777)
        }
        _ => return Err(stale()),
    };
    write_atomic(&path, source.as_bytes(), mode)?;
    let mut journal = Journal::default();
    journal.0.push(match old {
        Some(bytes) => Undo::Write {
            path: path.clone(),
            bytes,
            mode,
        },
        None => Undo::Remove(path.clone()),
    });
    let checkpoint = match git::checkpoint_paths(
        &root,
        "Updated project settings",
        &[PathBuf::from(SETTINGS_FILE)],
    )
    .await
    {
        Ok(cp) => cp,
        Err(e) => return Err(journal.fail(e)),
    };
    emit_checkpoint(state, &req.project, checkpoint);
    read(state, &req.project).await
}

pub fn describe(errors: &FieldErrors) -> String {
    if errors.is_empty() {
        return format!("{SETTINGS_FILE}: invalid settings");
    }
    let parts: Vec<String> = errors
        .iter()
        .map(|(field, message)| match field.as_str() {
            "_file" => message.clone(),
            _ => format!("{field} {message}"),
        })
        .collect();
    format!("{SETTINGS_FILE}: {}", parts.join("; "))
}

/// The configured `installDir` when that field validates on its own, else
/// the project dir. Used where settings as a whole may be invalid (the
/// dependency poll) so a valid install folder is still honoured.
pub fn install_dir_lenient(project: &Path) -> PathBuf {
    let fallback = || project.to_path_buf();
    let Ok(content) = std::fs::read_to_string(project.join(SETTINGS_FILE)) else {
        return fallback();
    };
    let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&content) else {
        return fallback();
    };
    let mut errors = FieldErrors::new();
    let Some(value) = string_field(&map, "installDir", &mut errors) else {
        return fallback();
    };
    match validate_dir(project, "installDir", &value, true, false, &mut errors) {
        Some(normalized) => join_rel(project, &normalized),
        None => fallback(),
    }
}

pub fn site_prefix(project: &Path) -> String {
    load(project).map(|s| s.site_prefix()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn write_settings(dir: &Path, content: &str) {
        fs::write(dir.join(SETTINGS_FILE), content).unwrap();
    }

    fn write_astro(dir: &Path, name: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(name), "export default {};\n").unwrap();
    }

    #[test]
    fn missing_file_gives_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(load(tmp.path()), Ok(ProjectSettings::defaults()));
        assert_eq!(
            ProjectSettings::defaults().dev,
            "npm run dev -- --port {port} --host {host} --ignore-lock"
        );
        assert_eq!(
            ProjectSettings::defaults().install,
            "npm install --no-audit --no-fund --no-progress"
        );
        assert_eq!(ProjectSettings::defaults().root, ".");
        assert_eq!(ProjectSettings::defaults().install_dir, ".");
    }

    #[test]
    fn missing_fields_fall_back() {
        let tmp = tempfile::tempdir().unwrap();
        write_settings(tmp.path(), r#"{"install":"pnpm install"}"#);
        let settings = load(tmp.path()).unwrap();
        assert_eq!(settings.install, "pnpm install");
        assert_eq!(settings.root, ".");
        assert_eq!(settings.dev, ProjectSettings::defaults().dev);
        assert_eq!(settings.install_dir, ".");
    }

    #[test]
    fn empty_fields_mean_unset() {
        let tmp = tempfile::tempdir().unwrap();
        write_settings(
            tmp.path(),
            r#"{"root":"","installDir":"","install":"","dev":""}"#,
        );
        assert_eq!(load(tmp.path()), Ok(ProjectSettings::defaults()));
        assert_eq!(
            ProjectSettings::defaults().site_dir(tmp.path()),
            tmp.path().to_path_buf()
        );
        assert_eq!(
            ProjectSettings::defaults().install_path(tmp.path()),
            tmp.path().to_path_buf()
        );
    }

    #[test]
    fn root_is_normalized() {
        let tmp = tempfile::tempdir().unwrap();
        write_astro(&tmp.path().join("apps/web"), "astro.config.mjs");
        for raw in ["./apps/web/", "apps/web"] {
            write_settings(tmp.path(), &format!(r#"{{"root":"{raw}"}}"#));
            let settings = load(tmp.path()).unwrap();
            assert_eq!(settings.root, "apps/web", "raw={raw}");
            assert_eq!(settings.site_prefix(), "apps/web/");
            assert_eq!(site_prefix(tmp.path()), "apps/web/");
            assert_eq!(settings.site_dir(tmp.path()), tmp.path().join("apps/web"));
        }
        // "." stays "." with an empty prefix.
        write_settings(tmp.path(), r#"{"root":"."}"#);
        // Root "." explicitly set requires an astro config at the root.
        write_astro(tmp.path(), "astro.config.mjs");
        let settings = load(tmp.path()).unwrap();
        assert_eq!(settings.root, ".");
        assert_eq!(settings.site_prefix(), "");
    }

    #[test]
    fn rejects_bad_paths() {
        let tmp = tempfile::tempdir().unwrap();
        for bad in ["/abs", "../x", "missing"] {
            write_settings(tmp.path(), &format!(r#"{{"root":"{bad}"}}"#));
            let err = load(tmp.path()).unwrap_err();
            assert!(err.contains_key("root"), "root={bad}: {err:?}");
            write_settings(tmp.path(), &format!(r#"{{"installDir":"{bad}"}}"#));
            let err = load(tmp.path()).unwrap_err();
            assert!(err.contains_key("installDir"), "installDir={bad}: {err:?}");
        }
    }

    #[test]
    fn rejects_symlink_escape() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        write_astro(outside.path(), "astro.config.mjs");
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.path(), tmp.path().join("apps")).unwrap();
        write_settings(tmp.path(), r#"{"root":"apps"}"#);
        let err = load(tmp.path()).unwrap_err();
        assert!(err.contains_key("root"), "{err:?}");
    }

    #[test]
    fn root_needs_astro_config() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("apps/web")).unwrap();
        write_settings(tmp.path(), r#"{"root":"apps/web"}"#);
        let err = load(tmp.path()).unwrap_err();
        assert!(err.contains_key("root"), "{err:?}");
        for name in ["astro.config.ts", "astro.config.mjs"] {
            // Remove any previous config, then add this one.
            for entry in fs::read_dir(tmp.path().join("apps/web")).unwrap() {
                let p = entry.unwrap().path();
                if p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("astro.config.")
                {
                    fs::remove_file(p).unwrap();
                }
            }
            fs::write(
                tmp.path().join("apps/web").join(name),
                "export default {};\n",
            )
            .unwrap();
            let settings = load(tmp.path()).unwrap();
            assert_eq!(settings.root, "apps/web", "config={name}");
        }
    }

    #[test]
    fn dev_needs_port() {
        let tmp = tempfile::tempdir().unwrap();
        write_settings(tmp.path(), r#"{"dev":"pnpm dev"}"#);
        let err = load(tmp.path()).unwrap_err();
        assert!(err.get("dev").unwrap().contains("{port}"), "{err:?}");
    }

    #[test]
    fn wrong_types_and_bad_json() {
        let tmp = tempfile::tempdir().unwrap();
        write_settings(tmp.path(), r#"{"root":1}"#);
        let err = load(tmp.path()).unwrap_err();
        assert!(err.contains_key("root"), "{err:?}");
        write_settings(tmp.path(), "not json");
        let err = load(tmp.path()).unwrap_err();
        assert!(err.contains_key("_file"), "{err:?}");
        write_settings(tmp.path(), "[]");
        let err = load(tmp.path()).unwrap_err();
        assert!(err.contains_key("_file"), "{err:?}");
    }

    #[test]
    fn dev_command_substitutes() {
        assert_eq!(
            ProjectSettings::defaults().dev_command(4567),
            "npm run dev -- --port 4567 --host 127.0.0.1 --ignore-lock"
        );
        let custom = ProjectSettings {
            dev: "run --port {port} --host {host} {other}".to_string(),
            ..ProjectSettings::defaults()
        };
        assert_eq!(
            custom.dev_command(4567),
            "run --port 4567 --host 127.0.0.1 {other}"
        );
    }

    #[test]
    fn install_dir_lenient_cases() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        assert_eq!(install_dir_lenient(dir), dir.to_path_buf());
        fs::create_dir_all(dir.join("vendor")).unwrap();
        // Valid installDir wins even when another field is invalid.
        write_settings(dir, r#"{"installDir":"vendor","dev":"x"}"#);
        assert_eq!(install_dir_lenient(dir), dir.join("vendor"));
        // Invalid installDir falls back to the project dir.
        write_settings(dir, r#"{"installDir":"missing"}"#);
        assert_eq!(install_dir_lenient(dir), dir.to_path_buf());
        write_settings(dir, r#"{"installDir":1}"#);
        assert_eq!(install_dir_lenient(dir), dir.to_path_buf());
        write_settings(dir, "not json");
        assert_eq!(install_dir_lenient(dir), dir.to_path_buf());
    }

    #[test]
    fn describe_joins() {
        let mut errors = FieldErrors::new();
        errors.insert("dev".to_string(), "must contain {port}".to_string());
        errors.insert("root".to_string(), "must exist".to_string());
        let text = describe(&errors);
        assert!(text.starts_with("orso.json: "), "{text}");
        assert!(text.contains("; "), "{text}");
        assert_eq!(text, "orso.json: dev must contain {port}; root must exist");
    }

    #[test]
    fn describe_file_error_without_key() {
        let mut errors = FieldErrors::new();
        errors.insert("_file".to_string(), "invalid JSON: eof".to_string());
        assert_eq!(describe(&errors), "orso.json: invalid JSON: eof");
    }
}
