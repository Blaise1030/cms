//! The skill package: parse a `SKILL.md` (YAML front matter plus a Markdown body), list the
//! installed skills in the project and global roots, and expose the built-in catalog skills.
//!
//! Front matter follows the same `---` rules as [`crate::design`]: it is present only when the
//! source opens with `---` and a later line is exactly `---`; the first such line closes it, so
//! a horizontal rule in the body is left alone. Unknown keys are kept so a round trip preserves
//! everything the editor does not understand.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::content::catalog;
use crate::content::lifecycle::{Journal, Undo};
use crate::content::mutation::{
    emit_checkpoint, lock_for, project_root, read_current, require_content_change, resolve, stale,
    write_atomic,
};
use crate::error::ApiError;
use crate::{git, state::AppState};

/// The file inside a skill directory that holds the skill's front matter and instructions.
pub const SKILL_FILE: &str = "SKILL.md";
/// Skills live at `<project>/.orso/skills/<id>/SKILL.md`; never `{projects-dir}/.orso/skills`.
pub const PROJECT_SKILLS_REL: &str = ".orso/skills";

/// Where a skill is installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillScope {
    Project,
    Global,
}

impl SkillScope {
    /// The serde value for this scope.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Global => "global",
        }
    }
}

/// A parsed, valid skill: enough to list it without loading its body.
#[derive(Debug, Clone, Serialize)]
pub struct SkillMeta {
    pub id: String,
    pub name: String,
    pub description: String,
    pub scope: SkillScope,
    pub path: PathBuf,
}

/// A fully read skill, including its body and a parse flag for the API layer.
#[derive(Debug, Clone, Serialize)]
pub struct SkillDoc {
    pub id: String,
    pub name: String,
    pub description: String,
    pub body: String,
    pub hash: String,
    pub scope: SkillScope,
    pub path: PathBuf,
    pub valid: bool,
    pub error: Option<String>,
}

/// The global skills root: an absolute path, defaulting under the projects directory.
pub fn default_global_skills_dir(projects_dir: &Path) -> PathBuf {
    projects_dir.join(".orso/global-skills")
}

fn invalid(message: impl std::fmt::Display) -> ApiError {
    ApiError::badreq(format!("SKILL.md front matter is invalid: {message}"))
        .with_reason("invalid_skill")
}

fn require_string(map: &Map<String, Value>, key: &str) -> Result<(), ApiError> {
    match map.get(key) {
        Some(Value::String(s)) if !s.is_empty() => Ok(()),
        _ => Err(invalid(format!("`{key}` must be a non-empty string"))),
    }
}

/// Split `source` into front-matter and the body, requiring string `name` and `description`.
pub fn split_skill(source: &str) -> Result<(Map<String, Value>, String), ApiError> {
    let Some(rest) = source
        .strip_prefix("---\r\n")
        .or_else(|| source.strip_prefix("---\n"))
    else {
        return Err(invalid("missing front matter"));
    };
    let mut start = 0;
    for line in rest.split_inclusive('\n') {
        let text = line
            .strip_suffix("\r\n")
            .or_else(|| line.strip_suffix('\n'))
            .unwrap_or(line);
        if text == "---" {
            let value = serde_yaml::from_str::<Value>(&rest[..start]).map_err(invalid)?;
            let Value::Object(map) = value else {
                return Err(invalid("expected a mapping"));
            };
            require_string(&map, "name")?;
            require_string(&map, "description")?;
            return Ok((map, rest[start + line.len()..].to_string()));
        }
        start += line.len();
    }
    Err(invalid("missing closing front matter"))
}

/// The inverse of [`split_skill`], mirroring [`crate::design::render`].
pub fn render_skill(frontmatter: &Map<String, Value>, body: &str) -> Result<String, ApiError> {
    if frontmatter.is_empty() {
        let opens_with_rule = body.starts_with("---\n") || body.starts_with("---\r\n");
        return Ok(if opens_with_rule {
            format!("---\n---\n{body}")
        } else {
            body.to_string()
        });
    }
    let yaml = serde_yaml::to_string(frontmatter).map_err(invalid)?;
    Ok(format!("---\n{yaml}---\n{body}"))
}

/// Parse a skill's front matter into a [`SkillMeta`]. On failure the returned meta has empty
/// `name`/`description`; this is a listing helper — prefer reading the full doc for the API.
pub fn parse_meta(id: &str, scope: SkillScope, path: &Path, source: &str) -> SkillMeta {
    let (name, description) = match split_skill(source) {
        Ok((map, _)) => (
            map["name"].as_str().unwrap_or_default().to_string(),
            map["description"].as_str().unwrap_or_default().to_string(),
        ),
        Err(_) => (String::new(), String::new()),
    };
    SkillMeta {
        id: id.to_string(),
        name,
        description,
        scope,
        path: path.to_path_buf(),
    }
}

/// List the installed skills under `root`, one per `<id>/SKILL.md` directory, skipping any
/// directory whose `SKILL.md` lacks the required keys. Sorted by id.
pub fn list_dir(root: &Path, scope: SkillScope) -> Vec<SkillMeta> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    let mut out = Vec::new();
    for dir in dirs {
        let path = dir.join(SKILL_FILE);
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        let id = dir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let meta = parse_meta(&id, scope, &path, &source);
        if meta.name.is_empty() || meta.description.is_empty() {
            continue;
        }
        out.push(meta);
    }
    out
}

fn project_rel_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

/// The agent-facing index of installed skills: empty when none, otherwise a `## Installed
/// skills` heading, one bullet per skill, and a closing reminder to read on demand.
pub fn skills_index(project_root: &Path, global_root: &Path) -> String {
    let project = list_dir(&project_root.join(PROJECT_SKILLS_REL), SkillScope::Project);
    let global = list_dir(global_root, SkillScope::Global);
    if project.is_empty() && global.is_empty() {
        return String::new();
    }
    let mut lines = vec!["## Installed skills".to_string()];
    for meta in project.iter().chain(global.iter()) {
        let path = match meta.scope {
            SkillScope::Project => project_rel_path(project_root, &meta.path),
            SkillScope::Global => meta.path.display().to_string(),
        };
        lines.push(format!(
            "- **{}** ({}): {} — `{}`",
            meta.id,
            meta.scope.as_str(),
            meta.description,
            path
        ));
    }
    lines.push(
        "Read a skill's SKILL.md when the task matches its description. Do not load every skill up front."
            .to_string(),
    );
    lines.join("\n")
}

const CATALOG_SEO_MD: &str = include_str!("catalog/seo/SKILL.md");
const CATALOG_DESIGN_MD: &str = include_str!("catalog/design/SKILL.md");

/// The built-in catalog skills as `(id, name, description)`.
pub fn catalog_entries() -> Vec<(String, String, String)> {
    [("seo", CATALOG_SEO_MD), ("design", CATALOG_DESIGN_MD)]
        .into_iter()
        .filter_map(|(id, md)| {
            let (map, _) = split_skill(md).ok()?;
            Some((
                id.to_string(),
                map["name"].as_str()?.to_string(),
                map["description"].as_str()?.to_string(),
            ))
        })
        .collect()
}

/// The raw markdown of a catalog skill, by id.
pub fn catalog_skill_md(id: &str) -> Option<&'static str> {
    match id {
        "seo" => Some(CATALOG_SEO_MD),
        "design" => Some(CATALOG_DESIGN_MD),
        _ => None,
    }
}

/// The configured global skills root and the default it falls back to.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsSettings {
    pub global_path: PathBuf,
    pub default_global_path: PathBuf,
}

/// `{projects_dir}/.orso/skills-settings.json`.
pub fn skills_settings_path(projects_dir: &Path) -> PathBuf {
    projects_dir.join(".orso/skills-settings.json")
}

/// The configured global skills root, or the default when unset, empty, or not absolute.
pub fn load_global_root(projects_dir: &Path) -> PathBuf {
    let default = default_global_skills_dir(projects_dir);
    let Ok(text) = std::fs::read_to_string(skills_settings_path(projects_dir)) else {
        return default;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return default;
    };
    if let Some(global_path) = value.get("globalPath").and_then(Value::as_str) {
        let path = PathBuf::from(global_path);
        if path.is_absolute() {
            return path;
        }
    }
    default
}

/// Persist `path` as the global skills root. The path must be absolute; its parent
/// directory is created on save.
pub fn save_global_root(projects_dir: &Path, path: &Path) -> Result<(), ApiError> {
    if !path.is_absolute() {
        return Err(ApiError::badreq("global skills path must be absolute"));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let settings_path = skills_settings_path(projects_dir);
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let value = json!({ "globalPath": path.to_string_lossy() });
    write_atomic(&settings_path, value.to_string().as_bytes(), 0o644)?;
    Ok(())
}

/// A built-in catalog skill and whether it is installed in each scope.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub installed_project: bool,
    pub installed_global: bool,
}

/// The `GET /api/skills` payload.
#[derive(Debug, Clone, Serialize)]
pub struct SkillsList {
    pub installed: Vec<SkillMeta>,
    pub catalog: Vec<CatalogItem>,
}

/// `PUT /api/skills/:id` body; the id comes from the URL path.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSkill {
    pub project: String,
    pub branch: Option<String>,
    pub scope: SkillScope,
    pub name: Option<String>,
    pub description: Option<String>,
    pub body: String,
    pub hash: String,
}

/// `POST /api/skills/install` body.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallSkill {
    pub project: String,
    pub branch: Option<String>,
    pub scope: SkillScope,
    pub source: SkillSource,
}

/// Where an installed skill comes from (externally tagged, camelCase fields).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillSource {
    Catalog { id: String },
    Import {
        #[serde(rename = "skillMd")]
        skill_md: String,
        #[serde(default)]
        id: Option<String>,
    },
    Blank { name: String, description: String },
    Copy {
        id: String,
        #[serde(rename = "fromScope")]
        from_scope: SkillScope,
    },
}

fn validate_skill_id(id: &str) -> Result<(), ApiError> {
    if id.is_empty() || id.starts_with('.') || id.contains(['/', '\\', '\0']) {
        return Err(ApiError::badreq("invalid skill id"));
    }
    Ok(())
}

/// Absolute path of a skill's `SKILL.md` in the given scope.
fn skill_path(
    root: &Path,
    global_root: &Path,
    id: &str,
    scope: SkillScope,
) -> Result<PathBuf, ApiError> {
    match scope {
        SkillScope::Project => resolve(root, &format!("{PROJECT_SKILLS_REL}/{id}/{SKILL_FILE}")),
        SkillScope::Global => Ok(global_root.join(id).join(SKILL_FILE)),
    }
}

/// A lowercase, hyphen-separated id derived from a display name.
fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for ch in name.chars() {
        let ch = ch.to_ascii_lowercase();
        slug.push(if ch.is_ascii_alphanumeric() { ch } else { '-' });
    }
    slug.split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn read_doc(id: &str, scope: SkillScope, path: &Path, source: &str) -> Result<SkillDoc, ApiError> {
    let (map, body) = split_skill(source)?;
    Ok(SkillDoc {
        id: id.to_string(),
        name: map["name"].as_str().unwrap_or_default().to_string(),
        description: map["description"].as_str().unwrap_or_default().to_string(),
        body,
        hash: catalog::hash(source),
        scope,
        path: path.to_path_buf(),
        valid: true,
        error: None,
    })
}

/// Installed skills plus the built-in catalog with per-scope install flags.
pub async fn list(state: &AppState, project: &str) -> Result<SkillsList, ApiError> {
    let root = project_root(state, project).await?;
    let global_root = load_global_root(&state.config.projects_dir);
    let project_skills = list_dir(&root.join(PROJECT_SKILLS_REL), SkillScope::Project);
    let global_skills = list_dir(&global_root, SkillScope::Global);
    let catalog = catalog_entries()
        .into_iter()
        .map(|(id, name, description)| CatalogItem {
            installed_project: project_skills.iter().any(|m| m.id == id),
            installed_global: global_skills.iter().any(|m| m.id == id),
            id,
            name,
            description,
        })
        .collect();
    let mut installed = project_skills;
    installed.extend(global_skills);
    installed.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(SkillsList { installed, catalog })
}

/// Read one installed skill; a missing file is `ENOENT`, an unparseable one `BADREQ`.
pub async fn get(
    state: &AppState,
    project: &str,
    id: &str,
    scope: SkillScope,
) -> Result<SkillDoc, ApiError> {
    validate_skill_id(id)?;
    let root = project_root(state, project).await?;
    let global_root = load_global_root(&state.config.projects_dir);
    let path = skill_path(&root, &global_root, id, scope)?;
    let source = read_current(&path)?;
    read_doc(id, scope, &path, &source)
}

/// Replace an installed skill's front matter and body. Project saves run through the
/// content change gate and checkpoint; global saves only enforce the stale hash.
pub async fn save(state: &AppState, id: &str, req: SaveSkill) -> Result<SkillDoc, ApiError> {
    validate_skill_id(id)?;
    let root = project_root(state, &req.project).await?;
    let global_root = load_global_root(&state.config.projects_dir);
    let path = skill_path(&root, &global_root, id, req.scope)?;
    let lock_dir = match req.scope {
        SkillScope::Project => root.clone(),
        SkillScope::Global => global_root.clone(),
    };
    let lock = lock_for(&lock_dir);
    let _guard = lock.lock().await;
    if req.scope == SkillScope::Project {
        let branch = req
            .branch
            .as_deref()
            .ok_or_else(|| ApiError::badreq("branch is required for project skills"))?;
        require_content_change(&root, branch).await?;
    }
    let old = read_current(&path)?;
    if catalog::hash(&old) != req.hash {
        return Err(stale());
    }
    let (mut map, _) = split_skill(&old)?;
    if let Some(name) = &req.name {
        map.insert("name".into(), json!(name));
    }
    if let Some(description) = &req.description {
        map.insert("description".into(), json!(description));
    }
    let source = render_skill(&map, &req.body)?;
    let mode = std::fs::metadata(&path)?.permissions().mode() & 0o7777;
    write_atomic(&path, source.as_bytes(), mode)?;
    let mut journal = Journal::default();
    journal.0.push(Undo::Write {
        path: path.clone(),
        bytes: old.clone(),
        mode,
    });
    if req.scope == SkillScope::Project {
        let rel = PathBuf::from(format!("{PROJECT_SKILLS_REL}/{id}/{SKILL_FILE}"));
        let checkpoint = match git::checkpoint_paths(&root, "Updated skill", &[rel]).await {
            Ok(cp) => cp,
            Err(e) => return Err(journal.fail(e)),
        };
        emit_checkpoint(state, &req.project, checkpoint);
    }
    read_doc(id, req.scope, &path, &source)
}

/// Install a skill from the catalog, an import, a blank scaffold, or a cross-scope copy.
pub async fn install(state: &AppState, req: InstallSkill) -> Result<SkillMeta, ApiError> {
    let root = project_root(state, &req.project).await?;
    let global_root = load_global_root(&state.config.projects_dir);
    let (id, md) = resolve_source(&root, &global_root, &req.source)?;
    validate_skill_id(&id)?;
    let path = skill_path(&root, &global_root, &id, req.scope)?;
    let lock_dir = match req.scope {
        SkillScope::Project => root.clone(),
        SkillScope::Global => global_root.clone(),
    };
    let lock = lock_for(&lock_dir);
    let _guard = lock.lock().await;
    if std::fs::symlink_metadata(&path).is_ok() {
        return Err(
            ApiError::conflict(format!("skill {id} is already installed")).with_reason("duplicate_slug"),
        );
    }
    if req.scope == SkillScope::Project {
        let branch = req
            .branch
            .as_deref()
            .ok_or_else(|| ApiError::badreq("branch is required for project skills"))?;
        require_content_change(&root, branch).await?;
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_atomic(&path, md.as_bytes(), 0o644)?;
    let mut journal = Journal::default();
    journal.0.push(Undo::Remove(path.clone()));
    if req.scope == SkillScope::Project {
        let rel = PathBuf::from(format!("{PROJECT_SKILLS_REL}/{id}/{SKILL_FILE}"));
        let checkpoint = match git::checkpoint_paths(&root, "Installed skill", &[rel]).await {
            Ok(cp) => cp,
            Err(e) => return Err(journal.fail(e)),
        };
        emit_checkpoint(state, &req.project, checkpoint);
    }
    Ok(parse_meta(&id, req.scope, &path, &md))
}

/// Remove an installed skill. Project removal runs the change gate and checkpoints.
pub async fn uninstall(
    state: &AppState,
    id: &str,
    project: &str,
    scope: SkillScope,
    branch: Option<&str>,
) -> Result<(), ApiError> {
    validate_skill_id(id)?;
    let root = project_root(state, project).await?;
    let global_root = load_global_root(&state.config.projects_dir);
    let path = skill_path(&root, &global_root, id, scope)?;
    let dir = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| global_root.join(id));
    let lock_dir = match scope {
        SkillScope::Project => root.clone(),
        SkillScope::Global => global_root.clone(),
    };
    let lock = lock_for(&lock_dir);
    let _guard = lock.lock().await;
    if std::fs::symlink_metadata(&path).is_err() {
        return Err(ApiError::enoent(format!("no such skill: {id}")));
    }
    if scope == SkillScope::Project {
        let branch = branch
            .ok_or_else(|| ApiError::badreq("branch is required for project skills"))?;
        require_content_change(&root, branch).await?;
    }
    std::fs::remove_file(&path)?;
    let _ = std::fs::remove_dir(&dir);
    if scope == SkillScope::Project {
        let rel = PathBuf::from(format!("{PROJECT_SKILLS_REL}/{id}/{SKILL_FILE}"));
        let checkpoint = git::checkpoint_paths(&root, "Removed skill", &[rel]).await?;
        emit_checkpoint(state, project, checkpoint);
    }
    Ok(())
}

/// Turn an install `source` into the skill id and its full `SKILL.md` markdown.
fn resolve_source(
    root: &Path,
    global_root: &Path,
    source: &SkillSource,
) -> Result<(String, String), ApiError> {
    match source {
        SkillSource::Catalog { id } => {
            let md = catalog_skill_md(id)
                .ok_or_else(|| ApiError::enoent(format!("no such catalog skill: {id}")))?;
            Ok((id.clone(), md.to_string()))
        }
        SkillSource::Import { skill_md, id } => {
            let (map, _) = split_skill(skill_md)?;
            let name = map["name"].as_str().unwrap_or_default().to_string();
            let id = id.clone().unwrap_or_else(|| slugify(&name));
            Ok((id, skill_md.clone()))
        }
        SkillSource::Blank { name, description } => {
            if name.trim().is_empty() || description.trim().is_empty() {
                return Err(ApiError::badreq("skill name and description are required"));
            }
            let mut map = Map::new();
            map.insert("name".into(), json!(name));
            map.insert("description".into(), json!(description));
            let md = render_skill(&map, "")?;
            Ok((slugify(name), md))
        }
        SkillSource::Copy { id, from_scope } => {
            let src = skill_path(root, global_root, id, *from_scope)?;
            let md = read_current(&src)?;
            split_skill(&md)?;
            Ok((id.clone(), md))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn skill_source(name: &str, description: &str) -> String {
        format!("---\nname: {name}\ndescription: {description}\n---\nbody text\n")
    }

    #[test]
    fn split_skill_happy_path() {
        let src = skill_source("seo", "SEO guidance");
        let (map, body) = split_skill(&src).unwrap();
        assert_eq!(map["name"], json!("seo"));
        assert_eq!(map["description"], json!("SEO guidance"));
        assert_eq!(body, "body text\n");
    }

    #[test]
    fn split_skill_requires_name() {
        let src = "---\ndescription: no name here\n---\nbody\n";
        let err = split_skill(src).unwrap_err();
        assert_eq!(err.reason, Some("invalid_skill"));
    }

    #[test]
    fn split_skill_body_may_contain_rule() {
        let src = "---\nname: seo\ndescription: SEO guidance\n---\nintro\n\n---\n\nafter rule\n";
        let (_, body) = split_skill(src).unwrap();
        assert_eq!(body, "intro\n\n---\n\nafter rule\n");
    }

    #[test]
    fn split_skill_preserves_unknown_keys_on_round_trip() {
        let src = "---\nname: seo\ndescription: SEO guidance\nversion: 2\n---\nbody\n";
        let (map, body) = split_skill(src).unwrap();
        assert_eq!(map["version"], json!(2));
        let rendered = render_skill(&map, &body).unwrap();
        let (again, body2) = split_skill(&rendered).unwrap();
        assert_eq!(again, map);
        assert_eq!(body2, body);
        assert_eq!(again["version"], json!(2));
    }

    #[test]
    fn list_dir_skips_invalid() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join(PROJECT_SKILLS_REL);
        std::fs::create_dir_all(root.join("seo")).unwrap();
        std::fs::create_dir_all(root.join("broken")).unwrap();
        std::fs::write(root.join("seo/SKILL.md"), skill_source("seo", "SEO guidance")).unwrap();
        std::fs::write(
            root.join("broken/SKILL.md"),
            "---\ndescription: no name\n---\nbody\n",
        )
        .unwrap();

        let metas = list_dir(&root, SkillScope::Project);
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].id, "seo");
        assert_eq!(metas[0].name, "seo");
        assert_eq!(metas[0].scope, SkillScope::Project);
        assert_eq!(metas[0].path, root.join("seo/SKILL.md"));
    }

    #[test]
    fn skills_index_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(
            skills_index(&tmp.path().join("proj"), &tmp.path().join("glob")),
            ""
        );
    }

    #[test]
    fn skills_index_project_and_global() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path().join("proj");
        let global_root = tmp.path().join("glob");
        let project_dir = project_root.join(PROJECT_SKILLS_REL);
        std::fs::create_dir_all(project_dir.join("seo")).unwrap();
        std::fs::create_dir_all(global_root.join("design")).unwrap();
        std::fs::write(
            project_dir.join("seo/SKILL.md"),
            skill_source("seo", "SEO guidance"),
        )
        .unwrap();
        std::fs::write(
            global_root.join("design/SKILL.md"),
            skill_source("design", "Design guidance"),
        )
        .unwrap();

        let expected = format!(
            "## Installed skills\n\
             - **seo** (project): SEO guidance — `.orso/skills/seo/SKILL.md`\n\
             - **design** (global): Design guidance — `{}`\n\
             Read a skill's SKILL.md when the task matches its description. Do not load every skill up front.",
            global_root.join("design/SKILL.md").display()
        );
        assert_eq!(skills_index(&project_root, &global_root), expected);
    }

    #[test]
    fn catalog_skill_seo_has_name_and_description() {
        let md = catalog_skill_md("seo").unwrap();
        assert!(md.contains("name:"));
        assert!(md.contains("description:"));
    }
}
