use crate::{
    core::{driver_kind::DriverKind, workspace::Workspace},
    error::KwsError,
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

pub mod condition;
pub mod driver_kind;
pub mod pane;
pub mod ready_when;
pub mod split_axis;
pub mod split_node;
pub mod tab;
pub mod timeout_action;
pub mod weight;
pub mod workspace;

use tab::Tab;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub workspace: Workspace,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(rename = "tab", default)]
    pub tabs: Vec<Tab>,
}

impl Config {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, KwsError> {
        let path: &Path = path.as_ref();

        let content: String = fs::read_to_string(path).map_err(|e| match e.kind() {
            ErrorKind::NotFound => KwsError::ConfigFileNotFound(path.to_path_buf()),
            _ => KwsError::SystemIo(e),
        })?;

        let mut config: Self =
            toml::from_str(&content).map_err(|e| KwsError::InvalidConfigFile {
                error: Box::new(e),
                raw_content: content.clone(),
                file_path: path.to_path_buf(),
            })?;

        config.validate()?;

        Ok(config)
    }

    fn validate(&mut self) -> Result<(), KwsError> {
        if self.workspace.driver == DriverKind::Konsole && !cfg!(target_os = "linux") {
            return Err(KwsError::UnsupportedDriver(self.workspace.driver));
        }

        let expanded: PathBuf = Workspace::expand_tilde(&self.workspace.base);

        self.workspace.base = expanded
            .canonicalize()
            .map_err(|_| KwsError::BaseDirectoryNotFound(expanded.clone()))?;

        let base: PathBuf = self.workspace.base.clone();

        for tab in &mut self.tabs {
            tab.validate(&base)?;
        }

        self.validate_depends_on()
    }

    fn validate_depends_on(&self) -> Result<(), KwsError> {
        crate::exec::graph::DependencyGraph::build(self)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        driver_kind::DriverKind, pane::Pane, tab::Tab, timeout_action::TimeoutAction,
        workspace::Workspace,
    };

    fn base_config(base: &str, tabs: Vec<Tab>) -> Config {
        Config {
            workspace: Workspace {
                name: "test".into(),
                base: PathBuf::from(base),
                driver: DriverKind::Konsole,
                on_timeout: TimeoutAction::Continue,
            },
            env: HashMap::new(),
            tabs,
        }
    }

    fn pane_with_area(area: &str, deps: Option<Vec<&str>>) -> Pane {
        Pane {
            area: Some(area.into()),
            run: Some("true".into()),
            depends_on: deps.map(|d| d.into_iter().map(str::to_string).collect()),
            hold: false,
            ready_when: None,
        }
    }

    fn tab_with_panes(panes: Vec<Pane>) -> Tab {
        Tab {
            title: "t".into(),
            cwd: PathBuf::from("/tmp"),
            panes,
            splits: HashMap::new(),
        }
    }

    #[test]
    fn valid_cross_tab_depends_on() {
        let config = base_config(
            "/tmp",
            vec![
                tab_with_panes(vec![pane_with_area("db", None)]),
                tab_with_panes(vec![pane_with_area("api", Some(vec!["db"]))]),
            ],
        );

        assert!(config.validate_depends_on().is_ok());
    }

    #[test]
    fn unknown_depends_on_rejected() {
        let config = base_config(
            "/tmp",
            vec![tab_with_panes(vec![pane_with_area(
                "api",
                Some(vec!["nonexistent"]),
            )])],
        );

        assert!(config.validate_depends_on().is_err());
    }

    #[test]
    fn missing_file_returns_error() {
        assert!(Config::load_from_file("/nonexistent/path/kws.toml").is_err());
    }

    #[test]
    fn valid_minimal_config() {
        let toml = r#"
            [workspace]
            name = "test"
            base = "/tmp"

            [[tab]]
            title = "main"

              [[tab.pane]]
              run = "echo hello"
        "#;

        let mut config: Config = toml::from_str(toml).unwrap();

        assert!(config.validate().is_ok());
        assert_eq!(config.workspace.name, "test");
        assert_eq!(config.tabs.len(), 1);
    }

    #[test]
    fn nonexistent_base_rejected() {
        let toml = r#"
            [workspace]
            name = "test"
            base = "/nonexistent/path/xyz"
        "#;

        let mut config: Config = toml::from_str(toml).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn invalid_toml_rejected() {
        assert!(Config::load_from_file("/dev/null").is_err());
    }
}
