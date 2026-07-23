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
