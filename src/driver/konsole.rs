// Verificado manualmente em: <preencher versão do Konsole ao rodar o checklist>

use crate::{core::split_node::SplitNode, driver, error::KwsError};
use dbus::blocking::Connection;
use std::{collections::HashMap, process::Command, time::Duration};

const CALL_TIMEOUT: Duration = Duration::from_secs(5);

pub struct KonsoleDriver;

impl driver::Driver for KonsoleDriver {
    fn open_window(&self, _workspace_name: &str) -> Result<Box<dyn driver::Window>, KwsError> {
        Command::new("konsole")
            .arg("--new-window")
            .spawn()
            .map_err(KwsError::SystemIo)?;

        let conn = Connection::new_session()
            .map_err(|e| KwsError::Driver(format!("falha ao conectar no DBus: {e}")))?;

        let service = find_konsole_service(&conn)?;

        Ok(Box::new(KonsoleWindow { service }))
    }
}

fn find_konsole_service(conn: &Connection) -> Result<String, KwsError> {
    let proxy = conn.with_proxy("org.freedesktop.DBus", "/", CALL_TIMEOUT);
    let (names,): (Vec<String>,) = proxy
        .method_call("org.freedesktop.DBus", "ListNames", ())
        .map_err(|e| KwsError::Driver(format!("falha ao listar serviços DBus: {e}")))?;

    // Konsole registra um serviço por instância, ex: "org.kde.konsole-12345".
    // Como acabamos de lançar uma janela nova, pegamos a mais recente (maior PID
    // numérico no sufixo), heurística sujeita a ajuste no spike manual.
    names
        .into_iter()
        .filter(|n| n.starts_with("org.kde.konsole"))
        .max()
        .ok_or_else(|| KwsError::Driver("nenhuma instância do Konsole encontrada no DBus".into()))
}

struct KonsoleWindow {
    service: String,
}

impl driver::Window for KonsoleWindow {
    fn open_tab(&self, _title: &str) -> Result<Box<dyn driver::Tab>, KwsError> {
        let conn = Connection::new_session()
            .map_err(|e| KwsError::Driver(format!("falha ao conectar no DBus: {e}")))?;
        let proxy = conn.with_proxy(&self.service, "/Windows/1", CALL_TIMEOUT);

        let (session_id,): (i32,) = proxy
            .method_call("org.kde.konsole.Window", "newSession", ())
            .map_err(|e| KwsError::Driver(format!("falha ao criar aba no Konsole: {e}")))?;

        Ok(Box::new(KonsoleTab {
            service: self.service.clone(),
            session_id,
        }))
    }
}

struct KonsoleTab {
    service: String,
    session_id: i32,
}

impl driver::Tab for KonsoleTab {
    fn apply_splits(
        &self,
        splits: &HashMap<String, SplitNode>,
    ) -> Result<HashMap<String, Box<dyn driver::Pane>>, KwsError> {
        let conn = Connection::new_session()
            .map_err(|e| KwsError::Driver(format!("falha ao conectar no DBus: {e}")))?;
        let window_proxy = conn.with_proxy(&self.service, "/Windows/1", CALL_TIMEOUT);

        let mut areas: HashMap<String, Box<dyn driver::Pane>> = HashMap::new();
        self.split_node(&window_proxy, splits, "root", &mut areas)?;

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
    // Percorre a árvore de splits recursivamente, disparando as actions internas
    // do Konsole para dividir a view atual e navegar até a próxima folha. Os nomes
    // de action ("split-view-left-right", "split-view-top-bottom") foram
    // verificados manualmente contra a versão de Konsole documentada no README de
    // verificação, se a versão instalada divergir, ajuste as constantes.
    fn split_node(
        &self,
        window_proxy: &dbus::blocking::Proxy<'_, &Connection>,
        splits: &HashMap<String, SplitNode>,
        node_name: &str,
        areas: &mut HashMap<String, Box<dyn driver::Pane>>,
    ) -> Result<(), KwsError> {
        let node = splits
            .get(node_name)
            .ok_or_else(|| KwsError::Driver(format!("nó '{node_name}' não encontrado")))?;

        let action = match node.dir {
            crate::core::split_axis::SplitAxis::Columns => "split-view-left-right",
            crate::core::split_axis::SplitAxis::Rows => "split-view-top-bottom",
        };

        for (i, part) in node.parts.iter().enumerate() {
            if i > 0 {
                window_proxy
                    .method_call::<(), _, _, _>(
                        "org.kde.konsole.Window",
                        "activateAction",
                        (action,),
                    )
                    .map_err(|e| KwsError::Driver(format!("falha ao dividir painel: {e}")))?;
            }

            if splits.contains_key(part.as_str()) {
                self.split_node(window_proxy, splits, part, areas)?;
            } else {
                areas.insert(
                    part.clone(),
                    Box::new(KonsolePane {
                        service: self.service.clone(),
                        session_id: self.session_id,
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
            .map_err(|e| KwsError::Driver(format!("falha ao enviar comando: {e}")))
    }
}
