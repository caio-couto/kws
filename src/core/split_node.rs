use crate::core::{split_axis::SplitAxis, weight::Weight};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SplitNode {
    pub dir: SplitAxis,
    #[serde(default)]
    pub ratio: Vec<Weight>,
    pub parts: Vec<String>,
}
