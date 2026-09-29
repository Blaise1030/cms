//! Embedded assets. Currently the scaffold template (`templates/astro-marketing/`);
//! the SPA (`web/dist`) is embedded in a later task.

use std::io;
use std::path::{Component, Path, PathBuf};

use rust_embed::RustEmbed;

/// The Astro scaffold copied into every new project. Build artifacts that may exist in a
/// developer checkout are excluded so they never leak into scaffolds.
#[derive(RustEmbed)]
#[folder = "templates/astro-marketing/"]
#[exclude = "node_modules/**"]
#[exclude = "dist/**"]
#[exclude = ".astro/**"]
#[exclude = ".DS_Store"]
pub struct Template;

/// Relative paths of every embedded template file (forward-slash separated).
pub fn template_paths() -> Vec<String> {
    let mut v: Vec<String> = Template::iter().map(|p| p.into_owned()).collect();
    v.sort();
    v
}

/// Write the embedded template into `dest` (created if missing). Existing files are overwritten.
pub fn extract_template(dest: &Path) -> io::Result<()> {
    for rel in Template::iter() {
        let file = Template::get(&rel)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, rel.to_string()))?;
        let target = safe_join(dest, &rel)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, file.data.as_ref())?;
    }
    Ok(())
}

/// Join an embedded relative path onto `base`, refusing anything that could escape it.
fn safe_join(base: &Path, rel: &str) -> io::Result<PathBuf> {
    let mut out = base.to_path_buf();
    for comp in Path::new(rel).components() {
        match comp {
            Component::Normal(c) => out.push(c),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unsafe template path: {rel}"),
                ))
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_contains_content_model_and_excludes_build_artifacts() {
        let paths = template_paths();
        for want in [
            "package.json",
            "package-lock.json",
            "AGENTS.md",
            ".gitignore",
            "astro.config.mjs",
            "src/content.config.ts",
            "src/content/site.json",
            "src/content/pages/home.json",
            "src/content/sections/hero.mdx",
            "src/pages/index.astro",
        ] {
            assert!(paths.iter().any(|p| p == want), "missing {want}: {paths:?}");
        }
        assert!(
            !paths
                .iter()
                .any(|p| p.starts_with("node_modules/") || p.starts_with("dist/")),
            "{paths:?}"
        );
    }

    #[test]
    fn safe_join_rejects_traversal() {
        assert!(safe_join(Path::new("/x"), "../evil").is_err());
        assert!(safe_join(Path::new("/x"), "/abs").is_err());
        assert!(safe_join(Path::new("/x"), "a/b.txt").is_ok());
    }
}
