// Verificado manualmente em: Konsole 26.08.0 (Arch Linux, Wayland)

use crate::{
    core::{split_axis::SplitAxis, split_node::SplitNode},
    driver,
    error::KwsError,
};
use dbus::blocking::Connection;
use std::{
    collections::HashMap,
    path::PathBuf,
    process::Command,
    sync::Mutex,
    time::{Duration, Instant},
};

const CALL_TIMEOUT: Duration = Duration::from_secs(5);
const OPEN_WINDOW_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_WINDOW_PROBE: i32 = 32;

const DBUS_ACCESS_DENIED: &str = "org.freedesktop.DBus.Error.AccessDenied";

const DBUS_SECURITY_HINT: &str = "\
o Konsole está com a API de DBus sensível a segurança desativada, e o kws depende dela pra \
rodar comandos nos painéis.

Habilite em: Konsole > Configurações > Configurar Konsole > Geral > 'Enable the security \
sensitive parts of the DBus API'

Ou direto pelo terminal:
  kwriteconfig6 --file konsolerc --group KonsoleWindow --key EnableSecuritySensitiveDBusAPI --type bool true

Depois, feche todas as janelas do Konsole e rode o kws de novo.";

pub struct KonsoleDriver {
    pub attach: bool,
}

impl driver::Driver for KonsoleDriver {
    fn open_window(&self, _workspace_name: &str) -> Result<Box<dyn driver::Window>, KwsError> {
        if !dbus_security_enabled() {
            return Err(KwsError::Driver(DBUS_SECURITY_HINT.into()));
        }

        let conn = Connection::new_session()
            .map_err(|e| KwsError::Driver(format!("falha ao conectar no DBus: {e}")))?;

        if self.attach
            && let Some((service, n)) = most_recent_window(&conn)
        {
            return Ok(Box::new(KonsoleWindow {
                service,
                window_path: format!("/Windows/{n}"),
                // Janela já existente e potencialmente cheia de abas do
                // usuário: não mexemos na aba inicial, cada tab configurada
                // ganha uma sessão nova mesmo.
                initial: Mutex::new(None),
            }));
        }

        // Konsole roda em modo single-instance: nossa nova invocação pode ser
        // absorvida por QUALQUER processo já rodando, não necessariamente o
        // "mais recente". Por isso comparamos as janelas de TODAS as
        // instâncias existentes, não só de uma escolhida por heurística.
        let before = snapshot_windows(&conn);

        Command::new("konsole")
            .spawn()
            .map_err(KwsError::SystemIo)?;

        let deadline = Instant::now() + OPEN_WINDOW_TIMEOUT;

        loop {
            for (service, windows) in snapshot_windows(&conn) {
                let before_windows = before.get(&service);
                let new_window = windows
                    .into_iter()
                    .find(|n| before_windows.is_none_or(|b| !b.contains(n)));

                if let Some(n) = new_window {
                    let window_path = format!("/Windows/{n}");
                    let proxy = conn.with_proxy(&service, &window_path, CALL_TIMEOUT);

                    // Konsole sempre cria sua própria aba padrão ao abrir uma
                    // janela do zero. Guardamos essa sessão pra a primeira aba
                    // configurada reaproveitá-la em vez de criar mais uma.
                    let initial = match (session_list(&proxy), view_hierarchy(&proxy)) {
                        (Ok(sessions), Ok(views)) if sessions.len() == 1 && views.len() == 1 => {
                            Some((sessions[0], views[0]))
                        }
                        _ => None,
                    };

                    return Ok(Box::new(KonsoleWindow {
                        service,
                        window_path,
                        initial: Mutex::new(initial),
                    }));
                }
            }

            if Instant::now() >= deadline {
                return Err(KwsError::Driver(
                    "timeout esperando o Konsole abrir uma nova janela".into(),
                ));
            }

            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

fn dbus_security_enabled() -> bool {
    let config_dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));

    config_dir
        .and_then(|dir| std::fs::read_to_string(dir.join("konsolerc")).ok())
        .map(|content| parse_dbus_security_enabled(&content))
        .unwrap_or(false)
}

fn parse_dbus_security_enabled(konsolerc: &str) -> bool {
    let mut in_konsole_window = false;

    for line in konsolerc.lines() {
        let line = line.trim();

        if let Some(section) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            in_konsole_window = section == "KonsoleWindow";
            continue;
        }

        if in_konsole_window
            && let Some(value) = line.strip_prefix("EnableSecuritySensitiveDBusAPI=")
        {
            return value.trim().eq_ignore_ascii_case("true");
        }
    }

    false
}

fn list_konsole_services(conn: &Connection) -> Vec<String> {
    let proxy = conn.with_proxy("org.freedesktop.DBus", "/", CALL_TIMEOUT);
    let result: Result<(Vec<String>,), _> =
        proxy.method_call("org.freedesktop.DBus", "ListNames", ());

    result
        .map(|(names,)| {
            names
                .into_iter()
                .filter(|n| n.starts_with("org.kde.konsole-"))
                .collect()
        })
        .unwrap_or_default()
}

// Escolhe uma janela existente pra anexar (--attach): entre todas as
// instâncias do Konsole rodando, prefere a de PID mais alto (a mais recente),
// e dentro dela a primeira janela viva.
fn most_recent_window(conn: &Connection) -> Option<(String, i32)> {
    list_konsole_services(conn)
        .into_iter()
        .filter_map(|service| {
            let pid: u32 = service.strip_prefix("org.kde.konsole-")?.parse().ok()?;
            Some((pid, service))
        })
        .max_by_key(|(pid, _)| *pid)
        .and_then(|(_, service)| {
            let window = list_window_numbers(conn, &service).into_iter().next()?;
            Some((service, window))
        })
}

fn snapshot_windows(conn: &Connection) -> HashMap<String, Vec<i32>> {
    list_konsole_services(conn)
        .into_iter()
        .map(|service| {
            let windows = list_window_numbers(conn, &service);
            (service, windows)
        })
        .collect()
}

fn list_window_numbers(conn: &Connection, service: &str) -> Vec<i32> {
    (1..=MAX_WINDOW_PROBE)
        .filter(|n| {
            let proxy = conn.with_proxy(service, format!("/Windows/{n}"), CALL_TIMEOUT);
            proxy
                .method_call::<(String,), _, _, _>(
                    "org.freedesktop.DBus.Introspectable",
                    "Introspect",
                    (),
                )
                .is_ok()
        })
        .collect()
}

fn session_list(
    window_proxy: &dbus::blocking::Proxy<'_, &Connection>,
) -> Result<Vec<i32>, KwsError> {
    let (list,): (Vec<String>,) = window_proxy
        .method_call("org.kde.konsole.Window", "sessionList", ())
        .map_err(|e| KwsError::Driver(format!("falha ao listar sessões: {e}")))?;

    Ok(list.iter().filter_map(|s| s.parse().ok()).collect())
}

// viewHierarchy() devolve uma árvore por aba, serializada como
// "(splitterId)[v1|v2|...]" ("[]" = lado a lado, "{}" = empilhado), onde os
// splitters aninhados reaparecem como "(id){...}"/"(id)[...]" dentro da
// árvore. Os inteiros "soltos" (fora de parênteses) são ids de
// TerminalDisplay, um espaço de ids totalmente separado dos ids de sessão
// usados em sendText/Sessions/N. Extrai só esses ids soltos, removendo antes
// os prefixos "(N)" de splitter.
fn leaf_view_ids(trees: &[String]) -> Result<Vec<i32>, KwsError> {
    let splitter_id = regex::Regex::new(r"\(\d+\)").expect("regex estática válida");
    let bare_id = regex::Regex::new(r"\d+").expect("regex estática válida");

    trees
        .iter()
        .flat_map(|tree| {
            let stripped = splitter_id.replace_all(tree, "");
            bare_id
                .find_iter(&stripped)
                .map(|m| m.as_str().to_string())
                .collect::<Vec<_>>()
        })
        .map(|s| {
            s.parse::<i32>()
                .map_err(|e| KwsError::Driver(format!("id de view inválido '{s}': {e}")))
        })
        .collect()
}

fn view_hierarchy(
    window_proxy: &dbus::blocking::Proxy<'_, &Connection>,
) -> Result<Vec<i32>, KwsError> {
    let (trees,): (Vec<String>,) = window_proxy
        .method_call("org.kde.konsole.Window", "viewHierarchy", ())
        .map_err(|e| KwsError::Driver(format!("falha ao ler o layout de abas: {e}")))?;

    leaf_view_ids(&trees)
}

struct KonsoleWindow {
    service: String,
    window_path: String,
    initial: Mutex<Option<(i32, i32)>>,
}

impl driver::Window for KonsoleWindow {
    fn open_tab(&self, _title: &str) -> Result<Box<dyn driver::Tab>, KwsError> {
        if let Some((session_id, view_id)) = self.initial.lock().unwrap().take() {
            return Ok(Box::new(KonsoleTab {
                service: self.service.clone(),
                window_path: self.window_path.clone(),
                session_id,
                view_id,
            }));
        }

        let conn = Connection::new_session()
            .map_err(|e| KwsError::Driver(format!("falha ao conectar no DBus: {e}")))?;
        let proxy = conn.with_proxy(&self.service, &self.window_path, CALL_TIMEOUT);

        let before_views = view_hierarchy(&proxy)?;

        let (session_id,): (i32,) = proxy
            .method_call("org.kde.konsole.Window", "newSession", ())
            .map_err(|e| KwsError::Driver(format!("falha ao criar aba no Konsole: {e}")))?;

        let after_views = view_hierarchy(&proxy)?;
        let view_id = after_views
            .into_iter()
            .find(|v| !before_views.contains(v))
            .ok_or_else(|| {
                KwsError::Driver("não foi possível identificar a view da aba nova".into())
            })?;

        Ok(Box::new(KonsoleTab {
            service: self.service.clone(),
            window_path: self.window_path.clone(),
            session_id,
            view_id,
        }))
    }
}

struct KonsoleTab {
    service: String,
    window_path: String,
    session_id: i32,
    view_id: i32,
}

impl driver::Tab for KonsoleTab {
    fn apply_splits(
        &self,
        splits: &HashMap<String, SplitNode>,
    ) -> Result<HashMap<String, Box<dyn driver::Pane>>, KwsError> {
        let conn = Connection::new_session()
            .map_err(|e| KwsError::Driver(format!("falha ao conectar no DBus: {e}")))?;
        let window_proxy = conn.with_proxy(&self.service, &self.window_path, CALL_TIMEOUT);

        let mut areas: HashMap<String, Box<dyn driver::Pane>> = HashMap::new();

        self.split_node(
            &window_proxy,
            splits,
            "root",
            self.view_id,
            self.session_id,
            &mut areas,
        )?;

        Ok(areas)
    }

    fn single_pane(&self) -> Result<Box<dyn driver::Pane>, KwsError> {
        Ok(Box::new(KonsolePane {
            service: self.service.clone(),
            session_id: self.session_id,
        }))
    }
}

impl KonsoleTab {
    // createSplit recebe o id da *view* (TerminalDisplay), não o id de
    // sessão, e não devolve nem um nem outro. Então a cada split
    // comparamos viewHierarchy()/sessionList() antes/depois pra descobrir
    // os dois ids da view nova.
    fn split_node(
        &self,
        window_proxy: &dbus::blocking::Proxy<'_, &Connection>,
        splits: &HashMap<String, SplitNode>,
        node_name: &str,
        view_id: i32,
        session_id: i32,
        areas: &mut HashMap<String, Box<dyn driver::Pane>>,
    ) -> Result<(), KwsError> {
        let node = splits
            .get(node_name)
            .ok_or_else(|| KwsError::Driver(format!("nó '{node_name}' não encontrado")))?;

        let horizontal = matches!(node.dir, SplitAxis::Columns);
        let last_index = node.parts.len().saturating_sub(1);
        let mut current_view = view_id;
        let mut current_session = session_id;

        for (i, part) in node.parts.iter().enumerate() {
            let (this_view, this_session) = if i == last_index {
                (current_view, current_session)
            } else {
                let before_views = view_hierarchy(window_proxy)?;
                let before_sessions = session_list(window_proxy)?;

                let (ok,): (bool,) = window_proxy
                    .method_call(
                        "org.kde.konsole.Window",
                        "createSplit",
                        (current_view, horizontal),
                    )
                    .map_err(|e| KwsError::Driver(format!("falha ao dividir painel: {e}")))?;

                if !ok {
                    return Err(KwsError::Driver(format!(
                        "Konsole recusou dividir o painel da view {current_view}"
                    )));
                }

                let new_view = view_hierarchy(window_proxy)?
                    .into_iter()
                    .find(|v| !before_views.contains(v))
                    .ok_or_else(|| {
                        KwsError::Driver(
                            "não foi possível identificar a view nova após o split".into(),
                        )
                    })?;
                let new_session = session_list(window_proxy)?
                    .into_iter()
                    .find(|s| !before_sessions.contains(s))
                    .ok_or_else(|| {
                        KwsError::Driver(
                            "não foi possível identificar a sessão nova após o split".into(),
                        )
                    })?;

                let this = (current_view, current_session);

                current_view = new_view;
                current_session = new_session;
                this
            };

            if splits.contains_key(part.as_str()) {
                self.split_node(window_proxy, splits, part, this_view, this_session, areas)?;
            } else {
                areas.insert(
                    part.clone(),
                    Box::new(KonsolePane {
                        service: self.service.clone(),
                        session_id: this_session,
                    }),
                );
            }
        }

        Ok(())
    }
}

struct KonsolePane {
    service: String,
    session_id: i32,
}

impl driver::Pane for KonsolePane {
    fn run(&self, command: &str) -> Result<(), KwsError> {
        let conn = Connection::new_session()
            .map_err(|e| KwsError::Driver(format!("falha ao conectar no DBus: {e}")))?;
        let path = format!("/Sessions/{}", self.session_id);
        let proxy = conn.with_proxy(&self.service, path, CALL_TIMEOUT);

        proxy
            .method_call::<(), _, _, _>(
                "org.kde.konsole.Session",
                "sendText",
                (format!("{command}\n"),),
            )
            .map_err(|e| {
                if e.name() == Some(DBUS_ACCESS_DENIED) {
                    KwsError::Driver(DBUS_SECURITY_HINT.into())
                } else {
                    KwsError::Driver(format!("falha ao enviar comando: {e}"))
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_enabled_flag() {
        let konsolerc = "[General]\nfoo=bar\n\n[KonsoleWindow]\nEnableSecuritySensitiveDBusAPI=true\n";
        assert!(parse_dbus_security_enabled(konsolerc));
    }

    #[test]
    fn detects_disabled_flag() {
        let konsolerc = "[KonsoleWindow]\nEnableSecuritySensitiveDBusAPI=false\n";
        assert!(!parse_dbus_security_enabled(konsolerc));
    }

    #[test]
    fn missing_key_defaults_to_disabled() {
        let konsolerc = "[KonsoleWindow]\nUseSingleInstance=true\n";
        assert!(!parse_dbus_security_enabled(konsolerc));
    }

    #[test]
    fn missing_section_defaults_to_disabled() {
        assert!(!parse_dbus_security_enabled(""));
    }

    #[test]
    fn key_outside_konsole_window_section_is_ignored() {
        let konsolerc = "[Other]\nEnableSecuritySensitiveDBusAPI=true\n\n[KonsoleWindow]\nfoo=bar\n";
        assert!(!parse_dbus_security_enabled(konsolerc));
    }
}
