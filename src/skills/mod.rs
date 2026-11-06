//! The skill package: parse a `SKILL.md` (YAML front matter plus a Markdown body), list the
//! installed skills in the project and global roots, and expose the built-in catalog skills.
//!
//! Front matter follows the same `---` rules as [`crate::design`]: it is present only when the
//! source opens with `---` and a later line is exactly `---`; the first such line closes it, so
//! a horizontal rule in the body is left alone. Unknown keys are kept so a round trip preserves
//! everything the editor does not understand.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::ApiError;

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
