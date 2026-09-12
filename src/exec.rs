pub mod graph;
pub mod readiness;
pub mod wrapper;

use crate::{
    core::{Config, ready_when::ReadyWhen, timeout_action::TimeoutAction},
    driver::{self, Driver},
    error::KwsError,
};
use graph::{DependencyGraph, PaneId};
use std::{
    collections::HashMap,
    fs,
    sync::{Condvar, Mutex},
    time::Duration,
};

#[derive(Debug, Default)]
pub struct Summary {
    pub ran: Vec<String>,
    pub failed: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaneStatus {
    Pending,
    Ready,
    Failed,
}

struct Shared {
    status: Mutex<HashMap<PaneId, PaneStatus>>,
    cv: Condvar,
}

impl Shared {
    fn wait_for_deps(&self, deps: &[PaneId]) -> bool {
        let mut guard = self.status.lock().unwrap();

        loop {
            let mut all_ready = true;

            for dep in deps {
                match guard.get(dep) {
                    Some(PaneStatus::Ready) => {}
                    Some(PaneStatus::Failed) => return false,
                    _ => all_ready = false,
                }
            }

            if all_ready {
                return true;
            }

            guard = self.cv.wait(guard).unwrap();
        }
    }

    fn set(&self, id: PaneId, status: PaneStatus) {
        self.status.lock().unwrap().insert(id, status);
        self.cv.notify_all();
    }
}

fn pane_label(tab_title: &str, area: Option<&str>) -> String {
    match area {
        Some(area) => format!("{tab_title}/{area}"),
        None => tab_title.to_string(),
    }
}

pub fn run(config: &Config, driver: &dyn Driver) -> Result<Summary, KwsError> {
    let graph = DependencyGraph::build(config)?;
    let window = driver.open_window(&config.workspace.name)?;

    let mut driver_panes: HashMap<PaneId, Box<dyn driver::Pane>> = HashMap::new();

    for (tab_index, tab) in config.tabs.iter().enumerate() {
        let tab_handle = window.open_tab(&tab.title)?;

        if tab.splits.is_empty() {
            let handle = tab_handle.single_pane()?;
            driver_panes.insert(
                PaneId {
                    tab_index,
                    pane_index: 0,
                },
                handle,
            );
        } else {
            let mut areas = tab_handle.apply_splits(&tab.splits)?;

            for (pane_index, pane) in tab.panes.iter().enumerate() {
                let area = pane
                    .area
                    .as_deref()
                    .expect("validado: area obrigatória com splits");
                let handle = areas.remove(area).ok_or_else(|| {
                    KwsError::Driver(format!("área '{area}' sem handle de painel"))
                })?;

                driver_panes.insert(
                    PaneId {
                        tab_index,
                        pane_index,
                    },
                    handle,
                );
            }
        }
    }

    let status: HashMap<PaneId, PaneStatus> = graph
        .pane_ids()
        .map(|id| (id, PaneStatus::Pending))
        .collect();

    let shared = Shared {
        status: Mutex::new(status),
        cv: Condvar::new(),
    };

    let ran: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let failed: Mutex<Vec<String>> = Mutex::new(Vec::new());

    std::thread::scope(|scope| {
        for id in graph.pane_ids() {
            let tab = &config.tabs[id.tab_index];
            let pane = &tab.panes[id.pane_index];
            let deps = graph.dependencies(id).to_vec();
            let driver_pane = driver_panes[&id].as_ref();
            let shared = &shared;
            let ran = &ran;
            let failed = &failed;
            let workspace_name = &config.workspace.name;
            let on_timeout = config.workspace.on_timeout;

            scope.spawn(move || {
                let label = pane_label(&tab.title, pane.area.as_deref());

                if !shared.wait_for_deps(&deps) {
                    shared.set(id, PaneStatus::Failed);
                    failed.lock().unwrap().push(label.clone());
                    let _ =
                        driver_pane.run("echo 'kws: painel pulado, dependência falhou'; exit 1");
                    return;
                }

                let (log_file, exit_file) = wrapper::paths_for(
                    &wrapper::default_base_dir(),
                    workspace_name,
                    &tab.title,
                    pane.area.as_deref(),
                );

                if let Some(dir) = log_file.parent() {
                    let _ = fs::create_dir_all(dir);
                }

                let run_cmd = pane.run.as_deref().expect("validado: run obrigatório");
                let wrapped = wrapper::wrap_command(&wrapper::WrapSpec {
                    run: run_cmd,
                    env: &config.env,
                    cwd: &tab.cwd,
                    log_file: &log_file,
                    exit_file: &exit_file,
                    hold: pane.hold,
                });

                if driver_pane.run(&wrapped).is_err() {
                    shared.set(id, PaneStatus::Failed);
                    failed.lock().unwrap().push(label);
                    return;
                }

                let ready = match &pane.ready_when {
                    None => true,
                    Some(rw) => {
                        let timeout = rw.timeout.as_deref().map(|t| {
                            ReadyWhen::parse_duration(t).expect("validado: timeout válido")
                        });

                        readiness::wait_ready(
                            &rw.condition,
                            timeout,
                            &exit_file,
                            &log_file,
                            Duration::from_millis(50),
                        )
                    }
                };

                if ready || matches!(on_timeout, TimeoutAction::Continue) {
                    shared.set(id, PaneStatus::Ready);
                    ran.lock().unwrap().push(label);
                } else {
                    shared.set(id, PaneStatus::Failed);
                    failed.lock().unwrap().push(label);
                }
            });
        }
    });

    Ok(Summary {
        ran: ran.into_inner().unwrap(),
        failed: failed.into_inner().unwrap(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{
            Config, driver_kind::DriverKind, pane::Pane, ready_when::ReadyWhen, tab::Tab,
            timeout_action::TimeoutAction, workspace::Workspace,
        },
        driver::local::LocalDriver,
    };
    use std::{collections::HashMap, path::PathBuf};

    fn config_with(on_timeout: TimeoutAction, tabs: Vec<Tab>) -> Config {
        Config {
            workspace: Workspace {
                name: format!("test-{}", std::process::id()),
                base: PathBuf::from("/tmp"),
                driver: DriverKind::Konsole,
                on_timeout,
            },
            env: HashMap::new(),
            tabs,
        }
    }

    fn tab_with(title: &str, panes: Vec<Pane>) -> Tab {
        Tab {
            title: title.into(),
            cwd: PathBuf::from("/tmp"),
            panes,
            splits: HashMap::new(),
        }
    }

    fn pane(cmd: &str, depends_on: Option<Vec<&str>>) -> Pane {
        Pane {
            area: Some(cmd.to_string()),
            run: Some(cmd.to_string()),
            depends_on: depends_on.map(|d| d.into_iter().map(str::to_string).collect()),
            hold: false,
            ready_when: None,
        }
    }

    #[test]
    fn independent_panes_all_run() {
        let cfg = config_with(
            TimeoutAction::Continue,
            vec![
                tab_with(
                    "a",
                    vec![Pane {
                        area: None,
                        ..pane("true", None)
                    }],
                ),
                tab_with(
                    "b",
                    vec![Pane {
                        area: None,
                        ..pane("true", None)
                    }],
                ),
            ],
        );

        let summary = run(&cfg, &LocalDriver).unwrap();

        assert_eq!(summary.ran.len(), 2);
        assert!(summary.failed.is_empty());
    }

    #[test]
    fn dependent_pane_fails_when_dependency_fails_others_still_run() {
        // "db" roda `true` (sai com 0), mas o ready_when exige exit 1, então
        // o polling de prontidão sempre estoura o timeout; on_timeout = Fail
        // faz "db" ser marcado como Failed e "api" (que depende dele) ser pulado.
        let db = Pane {
            area: Some("db".into()),
            ready_when: Some(ReadyWhen {
                condition: crate::core::condition::Condition::Exit(1),
                timeout: Some("1s".into()),
            }),
            ..pane("true", None)
        };
        let api = Pane {
            area: Some("api".into()),
            ..pane("true", Some(vec!["db"]))
        };
        let cfg = config_with(
            TimeoutAction::Fail,
            vec![tab_with("db", vec![db]), tab_with("api", vec![api])],
        );

        let summary = run(&cfg, &LocalDriver).unwrap();

        assert!(summary.failed.contains(&"db/db".to_string()));
        assert!(summary.failed.contains(&"api/api".to_string()));
    }
}
