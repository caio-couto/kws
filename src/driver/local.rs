use crate::{core::split_node::SplitNode, driver, error::KwsError};
use std::{collections::HashMap, process::Command};

pub struct LocalDriver;

impl driver::Driver for LocalDriver {
    fn open_window(&self, _workspace_name: &str) -> Result<Box<dyn driver::Window>, KwsError> {
        Ok(Box::new(LocalWindow))
    }
}

struct LocalWindow;

impl driver::Window for LocalWindow {
    fn open_tab(&self, _title: &str) -> Result<Box<dyn driver::Tab>, KwsError> {
        Ok(Box::new(LocalTab))
    }
}

struct LocalTab;

impl driver::Tab for LocalTab {
    fn apply_splits(
        &self,
        splits: &HashMap<String, SplitNode>,
    ) -> Result<HashMap<String, Box<dyn driver::Pane>>, KwsError> {
        let mut areas: HashMap<String, Box<dyn driver::Pane>> = HashMap::new();

        for node in splits.values() {
            for part in &node.parts {
                if !splits.contains_key(part) {
                    areas.insert(part.clone(), Box::new(LocalPane) as Box<dyn driver::Pane>);
                }
            }
        }

        Ok(areas)
    }

    fn single_pane(&self) -> Result<Box<dyn driver::Pane>, KwsError> {
        Ok(Box::new(LocalPane))
    }
}

struct LocalPane;

impl driver::Pane for LocalPane {
    fn run(&self, command: &str) -> Result<(), KwsError> {
        Command::new("sh")
            .arg("-c")
            .arg(command)
            .spawn()
            .map_err(KwsError::SystemIo)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::Driver;
    use std::{thread::sleep, time::Duration};
    use tempfile::tempdir;

    #[test]
    fn runs_command_as_real_local_process() {
        let dir = tempdir().unwrap();
        let marker = dir.path().join("ran");

        let driver = LocalDriver;
        let window = driver.open_window("test").unwrap();
        let tab = window.open_tab("main").unwrap();
        let pane = tab.single_pane().unwrap();

        pane.run(&format!("touch {}", marker.display())).unwrap();

        for _ in 0..50 {
            if marker.exists() {
                break;
            }
            sleep(Duration::from_millis(20));
        }

        assert!(marker.exists());
    }

    #[test]
    fn apply_splits_returns_one_pane_per_leaf() {
        let driver = LocalDriver;
        let window = driver.open_window("test").unwrap();
        let tab = window.open_tab("main").unwrap();

        let mut splits = HashMap::new();
        splits.insert(
            "root".to_string(),
            SplitNode {
                dir: crate::core::split_axis::SplitAxis::Columns,
                ratio: vec![],
                parts: vec!["left".into(), "right".into()],
            },
        );

        let panes = tab.apply_splits(&splits).unwrap();

        assert_eq!(panes.len(), 2);
        assert!(panes.contains_key("left"));
        assert!(panes.contains_key("right"));
    }
}
