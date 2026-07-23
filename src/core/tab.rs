use crate::{
    core::{pane::Pane, split_node::SplitNode, weight::Weight},
    error::KwsError,
};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tab {
    pub title: String,
    #[serde(default)]
    pub cwd: PathBuf,
    #[serde(rename = "pane", default)]
    pub panes: Vec<Pane>,
    #[serde(rename = "split", default)]
    pub splits: HashMap<String, SplitNode>,
}

impl Tab {
    pub(super) fn validate(&mut self, base: &Path) -> Result<(), KwsError> {
        let cwd_abs: PathBuf = base.join(&self.cwd);

        self.cwd = cwd_abs
            .canonicalize()
            .map_err(|_| KwsError::BaseDirectoryNotFound(cwd_abs.clone()))?;

        let leaf_areas: Vec<String> = if self.splits.is_empty() {
            if self.panes.len() > 1 {
                return Err(KwsError::ValidationError(format!(
                    "aba '{}' sem splits não pode ter mais de 1 painel, tem {}",
                    self.title,
                    self.panes.len()
                )));
            }

            vec![]
        } else {
            self.validate_splits()?
        };

        self.validate_panes(&leaf_areas)?;
        self.validate_ready_when()
    }

    fn validate_splits(&self) -> Result<Vec<String>, KwsError> {
        if !self.splits.contains_key("root") {
            return Err(KwsError::ValidationError(format!(
                "aba '{}' define splits mas não tem nó 'root'",
                self.title
            )));
        }

        let mut leaves: Vec<String> = vec![];
        let mut visited: HashSet<String> = HashSet::new();

        Self::collect_leaves(&self.splits, "root", &mut leaves, &mut visited, &self.title)?;

        for (key, node) in &self.splits {
            if node.parts.is_empty() {
                return Err(KwsError::ValidationError(format!(
                    "nó '{key}' na aba '{}' não tem partes",
                    self.title
                )));
            }

            if !node.ratio.is_empty() && node.ratio.len() != node.parts.len() {
                return Err(KwsError::ValidationError(format!(
                    "nó '{key}' na aba '{}': {} proporções para {} partes",
                    self.title,
                    node.ratio.len(),
                    node.parts.len()
                )));
            }

            let all_percent: bool = node.ratio.iter().all(|w| matches!(w, Weight::Percent(_)));

            if all_percent && !node.ratio.is_empty() {
                let sum: u32 = node
                    .ratio
                    .iter()
                    .map(|w| {
                        if let Weight::Percent(p) = w {
                            *p as u32
                        } else {
                            0
                        }
                    })
                    .sum();

                if sum != 100 {
                    return Err(KwsError::ValidationError(format!(
                        "nó '{key}' na aba '{}': proporções somam {sum}%, esperado 100%",
                        self.title
                    )));
                }
            }
        }

        for leaf in &leaves {
            let n: usize = self
                .panes
                .iter()
                .filter(|p| p.area.as_deref() == Some(leaf))
                .count();

            match n {
                0 => {
                    return Err(KwsError::ValidationError(format!(
                        "área '{leaf}' na aba '{}' não tem painel associado",
                        self.title
                    )));
                }
                1 => {}
                _ => {
                    return Err(KwsError::ValidationError(format!(
                        "área '{leaf}' na aba '{}' tem {n} painéis (esperado 1)",
                        self.title
                    )));
                }
            }
        }

        Ok(leaves)
    }

    fn collect_leaves(
        splits: &HashMap<String, SplitNode>,
        node: &str,
        leaves: &mut Vec<String>,
        visited: &mut HashSet<String>,
        tab_title: &str,
    ) -> Result<(), KwsError> {
        if !visited.insert(node.to_string()) {
            return Err(KwsError::ValidationError(format!(
                "ciclo nos splits da aba '{tab_title}': nó '{node}' visitado mais de uma vez"
            )));
        }

        let split: &SplitNode = splits.get(node).ok_or_else(|| {
            KwsError::ValidationError(format!(
                "nó '{node}' referenciado mas não definido na aba '{tab_title}'"
            ))
        })?;

        for part in &split.parts {
            if splits.contains_key(part.as_str()) {
                Self::collect_leaves(splits, part, leaves, visited, tab_title)?;
            } else {
                leaves.push(part.clone());
            }
        }

        Ok(())
    }

    fn validate_panes(&self, leaf_areas: &[String]) -> Result<(), KwsError> {
        if leaf_areas.is_empty() {
            return Ok(());
        }

        for pane in &self.panes {
            let area: &str = pane.area.as_deref().ok_or_else(|| {
                KwsError::ValidationError(format!(
                    "painel na aba '{}': 'area' obrigatório em abas com splits",
                    self.title
                ))
            })?;

            if !leaf_areas.iter().any(|a| a == area) {
                return Err(KwsError::ValidationError(format!(
                    "área '{area}' na aba '{}' não corresponde a nenhuma área folha: {leaf_areas:?}",
                    self.title
                )));
            }
        }
        Ok(())
    }

    fn validate_ready_when(&self) -> Result<(), KwsError> {
        for pane in &self.panes {
            if let Some(rw) = &pane.ready_when {
                rw.validate(&self.title)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        condition::Condition, ready_when::ReadyWhen, split_axis::SplitAxis, weight::Weight,
    };

    fn pane(area: &str) -> Pane {
        Pane {
            area: Some(area.into()),
            run: None,
            depends_on: None,
            hold: false,
            ready_when: None,
        }
    }

    fn split(dir: SplitAxis, parts: &[&str]) -> SplitNode {
        SplitNode {
            dir,
            ratio: vec![],
            parts: parts.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn tab_with_splits(splits: HashMap<String, SplitNode>, panes: Vec<Pane>) -> Tab {
        Tab {
            title: "test".into(),
            cwd: PathBuf::new(),
            panes,
            splits,
        }
    }

    #[test]
    fn missing_root_rejected() {
        let tab = tab_with_splits(
            HashMap::from([("other".into(), split(SplitAxis::Rows, &["a", "b"]))]),
            vec![pane("a"), pane("b")],
        );

        assert!(tab.validate_splits().is_err());
    }

    #[test]
    fn cycle_detected() {
        let tab = tab_with_splits(
            HashMap::from([
                ("root".into(), split(SplitAxis::Columns, &["left", "right"])),
                ("left".into(), split(SplitAxis::Rows, &["root", "a"])),
            ]),
            vec![pane("a")],
        );

        assert!(tab.validate_splits().is_err());
    }

    #[test]
    fn leaf_without_pane_rejected() {
        let tab = tab_with_splits(
            HashMap::from([("root".into(), split(SplitAxis::Columns, &["a", "b"]))]),
            vec![pane("a")],
        );

        assert!(tab.validate_splits().is_err());
    }

    #[test]
    fn duplicate_pane_for_leaf_rejected() {
        let tab = tab_with_splits(
            HashMap::from([("root".into(), split(SplitAxis::Columns, &["a", "b"]))]),
            vec![pane("a"), pane("b"), pane("b")],
        );

        assert!(tab.validate_splits().is_err());
    }

    #[test]
    fn ratio_length_mismatch_rejected() {
        let tab = tab_with_splits(
            HashMap::from([(
                "root".into(),
                SplitNode {
                    dir: SplitAxis::Columns,
                    ratio: vec![Weight::Percent(60), Weight::Percent(40)],
                    parts: vec!["a".into(), "b".into(), "c".into()],
                },
            )]),
            vec![pane("a"), pane("b"), pane("c")],
        );

        assert!(tab.validate_splits().is_err());
    }

    #[test]
    fn percent_ratio_not_summing_to_100_rejected() {
        let tab = tab_with_splits(
            HashMap::from([(
                "root".into(),
                SplitNode {
                    dir: SplitAxis::Columns,
                    ratio: vec![Weight::Percent(60), Weight::Percent(30)],
                    parts: vec!["a".into(), "b".into()],
                },
            )]),
            vec![pane("a"), pane("b")],
        );

        assert!(tab.validate_splits().is_err());
    }

    #[test]
    fn valid_splits_returns_leaves() {
        let tab = tab_with_splits(
            HashMap::from([
                ("root".into(), split(SplitAxis::Columns, &["a", "right"])),
                ("right".into(), split(SplitAxis::Rows, &["b", "c"])),
            ]),
            vec![pane("a"), pane("b"), pane("c")],
        );

        let leaves = tab.validate_splits().unwrap();

        assert_eq!(leaves.len(), 3);
        assert!(leaves.contains(&"a".to_string()));
        assert!(leaves.contains(&"b".to_string()));
        assert!(leaves.contains(&"c".to_string()));
    }

    #[test]
    fn pane_missing_area_in_split_tab_rejected() {
        let tab = Tab {
            title: "test".into(),
            cwd: PathBuf::new(),
            panes: vec![Pane {
                area: None,
                run: None,
                depends_on: None,
                hold: false,
                ready_when: None,
            }],
            splits: HashMap::new(),
        };

        let leaf_areas = vec!["db".to_string()];

        assert!(tab.validate_panes(&leaf_areas).is_err());
    }

    #[test]
    fn pane_with_unknown_area_rejected() {
        let tab = tab_with_splits(HashMap::new(), vec![pane("unknown")]);

        assert!(tab.validate_panes(&["db".to_string()]).is_err());
    }

    #[test]
    fn single_pane_tab_skips_area_validation() {
        let tab = tab_with_splits(
            HashMap::new(),
            vec![Pane {
                area: None,
                run: None,
                depends_on: None,
                hold: false,
                ready_when: None,
            }],
        );

        assert!(tab.validate_panes(&[]).is_ok());
    }

    #[test]
    fn invalid_url_in_pane_rejected() {
        let tab = Tab {
            title: "test".into(),
            cwd: PathBuf::new(),
            panes: vec![Pane {
                area: None,
                run: None,
                depends_on: None,
                hold: false,
                ready_when: Some(ReadyWhen {
                    condition: Condition::Http("not-a-url".into()),
                    timeout: None,
                }),
            }],

            splits: HashMap::new(),
        };

        assert!(tab.validate_ready_when().is_err());
    }

    #[test]
    fn invalid_timeout_in_pane_rejected() {
        let tab = Tab {
            title: "test".into(),
            cwd: PathBuf::new(),
            panes: vec![Pane {
                area: None,
                run: None,
                depends_on: None,
                hold: false,
                ready_when: Some(ReadyWhen {
                    condition: Condition::Port(5432),
                    timeout: Some("bad".into()),
                }),
            }],
            splits: HashMap::new(),
        };
        assert!(tab.validate_ready_when().is_err());
    }

    #[test]
    fn nonexistent_cwd_rejected() {
        let mut tab = Tab {
            title: "test".into(),
            cwd: PathBuf::from("nonexistent_dir_xyz"),
            panes: vec![],
            splits: HashMap::new(),
        };

        assert!(tab.validate(Path::new("/tmp")).is_err());
    }

    #[test]
    fn too_many_panes_without_splits_rejected() {
        let mut tab = Tab {
            title: "test".into(),
            cwd: PathBuf::new(),
            panes: vec![pane("a"), pane("b")],
            splits: HashMap::new(),
        };

        assert!(tab.validate(Path::new("/tmp")).is_err());
    }

    #[test]
    fn valid_single_pane_tab() {
        let mut tab = Tab {
            title: "test".into(),
            cwd: PathBuf::new(),
            panes: vec![Pane {
                area: None,
                run: None,
                depends_on: None,
                hold: false,
                ready_when: None,
            }],
            splits: HashMap::new(),
        };

        assert!(tab.validate(Path::new("/tmp")).is_ok());
    }
}
