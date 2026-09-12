use crate::{core::split_node::SplitNode, error::KwsError};
use std::collections::HashMap;

pub trait Driver: Sync {
    fn open_window(&self, workspace_name: &str) -> Result<Box<dyn Window>, KwsError>;
}

pub trait Window: Sync {
    fn open_tab(&self, title: &str) -> Result<Box<dyn Tab>, KwsError>;
}

pub trait Tab: Sync {
    fn apply_splits(
        &self,
        splits: &HashMap<String, SplitNode>,
    ) -> Result<HashMap<String, Box<dyn Pane>>, KwsError>;

    fn single_pane(&self) -> Result<Box<dyn Pane>, KwsError>;
}

pub trait Pane: Sync {
    fn run(&self, command: &str) -> Result<(), KwsError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoopPane;

    impl Pane for NoopPane {
        fn run(&self, _command: &str) -> Result<(), KwsError> {
            Ok(())
        }
    }

    struct NoopTab;
    impl Tab for NoopTab {
        fn apply_splits(
            &self,
            _splits: &HashMap<String, SplitNode>,
        ) -> Result<HashMap<String, Box<dyn Pane>>, KwsError> {
            Ok(HashMap::new())
        }

        fn single_pane(&self) -> Result<Box<dyn Pane>, KwsError> {
            Ok(Box::new(NoopPane))
        }
    }

    struct NoopWindow;
    impl Window for NoopWindow {
        fn open_tab(&self, _title: &str) -> Result<Box<dyn Tab>, KwsError> {
            Ok(Box::new(NoopTab))
        }
    }

    struct NoopDriver;
    impl Driver for NoopDriver {
        fn open_window(&self, _workspace_name: &str) -> Result<Box<dyn Window>, KwsError> {
            Ok(Box::new(NoopWindow))
        }
    }

    #[test]
    fn traits_are_object_safe_and_composable() {
        let driver: Box<dyn Driver> = Box::new(NoopDriver);
        let window = driver.open_window("test").unwrap();
        let tab = window.open_tab("main").unwrap();
        let pane = tab.single_pane().unwrap();

        assert!(pane.run("true").is_ok());
    }
}
