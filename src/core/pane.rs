use crate::core::ready_when::ReadyWhen;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pane {
    pub area: Option<String>,
    pub run: Option<String>,
    pub depends_on: Option<Vec<String>>,
    #[serde(default)]
    pub hold: bool,
    pub ready_when: Option<ReadyWhen>,
}
