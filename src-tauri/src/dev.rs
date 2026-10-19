//! `just desktop-dev`: the main window shows Vite's hot-reloading UI, and the daemon listens on
//! the port Vite proxies `/api`, `/ws` and `/preview` to. Driven by env vars that only debug
//! builds read, so release builds always run the bundled sidecar and show its UI.

use tauri::Url;

/// Vite UI origin, e.g. `http://localhost:5173`.
const UI_ENV: &str = "ORSO_DEV_UI";
/// Daemon binary to run instead of the bundled sidecar, e.g. `target/debug/orso`.
const DAEMON_ENV: &str = "ORSO_DEV_DAEMON";
/// The daemon port Vite proxies to; `web/vite.config.ts` reads the same variable.
const PORT_ENV: &str = "ORSO_DAEMON_PORT";
const DEFAULT_PORT: &str = "7800";

#[derive(Debug, PartialEq, Eq)]
pub struct Plan {
    /// `None` runs the bundled sidecar.
    pub daemon: Option<String>,
    pub port: String,
    /// Where the main window goes instead of the daemon; it gets the daemon's token.
    pub ui: Option<Url>,
}

pub fn plan() -> Plan {
    plan_from(|name| std::env::var(name).ok(), cfg!(debug_assertions))
}

fn plan_from(env: impl Fn(&str) -> Option<String>, dev_build: bool) -> Plan {
    let var = |name| env(name).filter(|v| dev_build && !v.is_empty());
    let ui = var(UI_ENV).and_then(|v| Url::parse(&v).ok());
    let port = match ui {
        Some(_) => var(PORT_ENV)
            .filter(|p| p.parse::<u16>().is_ok())
            .unwrap_or_else(|| DEFAULT_PORT.into()),
        None => "0".into(),
    };
    Plan {
        daemon: var(DAEMON_ENV),
        port,
        ui,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev_env(name: &str) -> Option<String> {
        match name {
            UI_ENV => Some("http://localhost:5174".into()),
            DAEMON_ENV => Some("/repo/target/debug/orso".into()),
            PORT_ENV => Some("7801".into()),
            _ => None,
        }
    }

    fn bundled() -> Plan {
        Plan {
            daemon: None,
            port: "0".into(),
            ui: None,
        }
    }

    #[test]
    fn release_builds_ignore_the_dev_env() {
        assert_eq!(plan_from(dev_env, false), bundled());
    }

    #[test]
    fn without_the_dev_env_debug_builds_run_like_release() {
        assert_eq!(plan_from(|_| None, true), bundled());
        assert_eq!(plan_from(|_| Some(String::new()), true), bundled());
    }

    #[test]
    fn dev_runs_the_given_daemon_on_vites_proxy_port_and_shows_vite() {
        assert_eq!(
            plan_from(dev_env, true),
            Plan {
                daemon: Some("/repo/target/debug/orso".into()),
                port: "7801".into(),
                ui: Some(Url::parse("http://localhost:5174").unwrap()),
            }
        );
    }

    #[test]
    fn the_daemon_port_defaults_to_vites_default_proxy_target() {
        let env = |name: &str| (name == UI_ENV).then(|| "http://localhost:5173".to_string());
        assert_eq!(plan_from(env, true).port, "7800");
        let vite = include_str!("../../web/vite.config.ts");
        assert!(vite.contains(&format!("{PORT_ENV} ?? \"7800\"")));
    }

    #[test]
    fn a_bad_daemon_port_falls_back_to_the_default() {
        let env = |name: &str| match name {
            UI_ENV => Some("http://localhost:5173".into()),
            PORT_ENV => Some("7800; rm -rf".into()),
            _ => None,
        };
        assert_eq!(plan_from(env, true).port, "7800");
    }
}
