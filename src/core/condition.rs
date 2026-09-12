use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Condition {
    Delay(String),
    Port(u16),
    Http(String),
    File(String),
    Exit(i32),
    Log(String),
}
