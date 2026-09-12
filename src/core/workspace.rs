use crate::core::{driver_kind::DriverKind, timeout_action::TimeoutAction};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workspace {
    pub name: String,
    pub base: PathBuf,
    #[serde(default)]
    pub driver: DriverKind,
    #[serde(default)]
    pub on_timeout: TimeoutAction,
}

impl Workspace {
    pub(crate) fn expand_tilde(path: &Path) -> PathBuf {
        let s: &str = path.to_str().unwrap_or("");

        if s == "~" {
            if let Some(home) = std::env::var_os("HOME") {
                return PathBuf::from(home);
            }
        } else if let Some(rest) = s.strip_prefix("~/")
            && let Some(home) = std::env::var_os("HOME")
        {
            return PathBuf::from(home).join(rest);
        }

        path.to_path_buf()
    }
}
