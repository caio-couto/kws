use crate::{core::Config, error::KwsError};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaneId {
    pub tab_index: usize,
    pub pane_index: usize,
}

#[derive(Debug)]
pub struct DependencyGraph {
    deps: HashMap<PaneId, Vec<PaneId>>,
}

impl DependencyGraph {
    pub fn build(config: &Config) -> Result<Self, KwsError> {
        let mut area_to_id: HashMap<&str, PaneId> = HashMap::new();

        for (tab_index, tab) in config.tabs.iter().enumerate() {
            for (pane_index, pane) in tab.panes.iter().enumerate() {
                if let Some(area) = pane.area.as_deref() {
                    area_to_id.insert(
                        area,
                        PaneId {
                            tab_index,
                            pane_index,
                        },
                    );
                }
            }
        }

        let mut deps: HashMap<PaneId, Vec<PaneId>> = HashMap::new();

        for (tab_index, tab) in config.tabs.iter().enumerate() {
            for (pane_index, pane) in tab.panes.iter().enumerate() {
                let id = PaneId {
                    tab_index,
                    pane_index,
                };
                let mut resolved = Vec::new();

                if let Some(dep_names) = &pane.depends_on {
                    for name in dep_names {
                        let dep_id = area_to_id.get(name.as_str()).copied().ok_or_else(|| {
                            KwsError::ValidationError(format!(
                                "dependência '{name}' não encontrada. Áreas disponíveis: {:?}",
                                area_to_id.keys().collect::<Vec<_>>()
                            ))
                        })?;
                        resolved.push(dep_id);
                    }
                }

                deps.insert(id, resolved);
            }
        }

        let graph = Self { deps };
        graph.check_cycles()?;
        Ok(graph)
    }

    fn check_cycles(&self) -> Result<(), KwsError> {
        #[derive(Clone, Copy, PartialEq)]
        enum Color {
            White,
            Gray,
            Black,
        }

        fn visit(
            id: PaneId,
            deps: &HashMap<PaneId, Vec<PaneId>>,
            color: &mut HashMap<PaneId, Color>,
        ) -> Result<(), KwsError> {
            color.insert(id, Color::Gray);

            for &dep in &deps[&id] {
                match color[&dep] {
                    Color::Gray => {
                        return Err(KwsError::ValidationError(
                            "ciclo detectado em depends_on".into(),
                        ));
                    }
                    Color::White => visit(dep, deps, color)?,
                    Color::Black => {}
                }
            }

            color.insert(id, Color::Black);
            Ok(())
        }

        let mut color: HashMap<PaneId, Color> =
            self.deps.keys().map(|id| (*id, Color::White)).collect();

        for id in self.deps.keys().copied().collect::<Vec<_>>() {
            if color[&id] == Color::White {
                visit(id, &self.deps, &mut color)?;
            }
        }

        Ok(())
    }

    pub fn dependencies(&self, id: PaneId) -> &[PaneId] {
        self.deps.get(&id).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn pane_ids(&self) -> impl Iterator<Item = PaneId> + '_ {
        self.deps.keys().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        driver_kind::DriverKind, pane::Pane, tab::Tab, timeout_action::TimeoutAction,
        workspace::Workspace,
    };
    use std::path::{Path, PathBuf};

    fn pane(area: &str, deps: Option<Vec<&str>>, run: &str) -> Pane {
        Pane {
            area: Some(area.into()),
            run: Some(run.into()),
            depends_on: deps.map(|d| d.into_iter().map(str::to_string).collect()),
            hold: false,
            ready_when: None,
        }
    }

    fn tab(title: &str, panes: Vec<Pane>) -> Tab {
        Tab {
            title: title.into(),
            cwd: PathBuf::new(),
            panes,
            splits: HashMap::new(),
        }
    }

    fn config(tabs: Vec<Tab>) -> Config {
        Config {
            workspace: Workspace {
                name: "test".into(),
                base: Path::new("/tmp").to_path_buf(),
                driver: DriverKind::Konsole,
                on_timeout: TimeoutAction::Continue,
            },
            env: HashMap::new(),
            tabs,
        }
    }

    #[test]
    fn no_deps_builds_empty_edges() {
        let cfg = config(vec![tab("t", vec![pane("a", None, "true")])]);
        let graph = DependencyGraph::build(&cfg).unwrap();
        let id = PaneId {
            tab_index: 0,
            pane_index: 0,
        };

        assert!(graph.dependencies(id).is_empty());
        assert_eq!(graph.pane_ids().count(), 1);
    }

    #[test]
    fn resolves_cross_tab_dependency() {
        let cfg = config(vec![
            tab("backend", vec![pane("db", None, "true")]),
            tab("frontend", vec![pane("api", Some(vec!["db"]), "true")]),
        ]);
        let graph = DependencyGraph::build(&cfg).unwrap();

        let api_id = PaneId {
            tab_index: 1,
            pane_index: 0,
        };
        let db_id = PaneId {
            tab_index: 0,
            pane_index: 0,
        };

        assert_eq!(graph.dependencies(api_id), &[db_id]);
    }

    #[test]
    fn unknown_dependency_rejected() {
        let cfg = config(vec![tab(
            "t",
            vec![pane("api", Some(vec!["nonexistent"]), "true")],
        )]);

        assert!(DependencyGraph::build(&cfg).is_err());
    }

    #[test]
    fn direct_cycle_rejected() {
        let cfg = config(vec![tab(
            "t",
            vec![
                pane("a", Some(vec!["b"]), "true"),
                pane("b", Some(vec!["a"]), "true"),
            ],
        )]);

        assert!(DependencyGraph::build(&cfg).is_err());
    }

    #[test]
    fn self_dependency_rejected() {
        let cfg = config(vec![tab("t", vec![pane("a", Some(vec!["a"]), "true")])]);

        assert!(DependencyGraph::build(&cfg).is_err());
    }

    #[test]
    fn diamond_shape_is_not_a_cycle() {
        let cfg = config(vec![tab(
            "t",
            vec![
                pane("a", None, "true"),
                pane("b", Some(vec!["a"]), "true"),
                pane("c", Some(vec!["a"]), "true"),
                pane("d", Some(vec!["b", "c"]), "true"),
            ],
        )]);

        assert!(DependencyGraph::build(&cfg).is_ok());
    }
}
