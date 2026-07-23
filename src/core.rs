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
        let all_areas: Vec<String> = self
            .tabs
            .iter()
            .flat_map(|t| t.panes.iter().filter_map(|p| p.area.clone()))
            .collect();

        for tab in &self.tabs {
            for pane in &tab.panes {
                if let Some(deps) = &pane.depends_on {
                    for dep in deps {
                        if !all_areas.contains(dep) {
                            return Err(KwsError::ValidationError(format!(
                                "dependência '{dep}' não encontrada. Áreas disponíveis: {all_areas:?}"
                            )));
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
