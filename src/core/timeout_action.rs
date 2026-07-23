use serde::Deserialize;

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeoutAction {
    Fail,
    #[default]
    Continue,
}
