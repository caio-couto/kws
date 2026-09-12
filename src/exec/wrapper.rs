use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub struct WrapSpec<'a> {
    pub run: &'a str,
    pub env: &'a HashMap<String, String>,
    pub cwd: &'a Path,
    pub log_file: &'a Path,
    pub exit_file: &'a Path,
    pub hold: bool,
}

pub fn wrap_command(spec: &WrapSpec) -> String {
    let exports: String = spec
        .env
        .iter()
        .map(|(k, v)| format!("export {k}={};\n", shell_quote(v)))
        .collect();

    let cwd = spec.cwd.display();
    let log_file = spec.log_file.display();
    let exit_file = spec.exit_file.display();
    let run = spec.run;

    let after_exit = if spec.hold {
        "exec \"$SHELL\""
    } else {
        "exit \"$ec\""
    };

    format!(
        "clear; if cd \"{cwd}\"; then {exports}{{ {{ {run}; }} > >(tee \"{log_file}\") 2>&1; }}; ec=$?; else ec=1; fi; echo \"$ec\" > \"{exit_file}\"; {after_exit}"
    )
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub fn default_base_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(|d| PathBuf::from(d).join("kws"))
        .unwrap_or_else(|| PathBuf::from("/tmp/kws"))
}

pub fn paths_for(
    base_dir: &Path,
    workspace: &str,
    tab_title: &str,
    area: Option<&str>,
) -> (PathBuf, PathBuf) {
    let area = area.unwrap_or("main");
    let base = base_dir.join(workspace).join(tab_title);
    (
        base.join(format!("{area}.log")),
        base.join(format!("{area}.exit")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_command_without_hold() {
        let env = HashMap::new();
        let spec = WrapSpec {
            run: "pnpm start:dev",
            env: &env,
            cwd: Path::new("/home/caio/proj/backend"),
            log_file: Path::new("/tmp/kws/w/backend/api.log"),
            exit_file: Path::new("/tmp/kws/w/backend/api.exit"),
            hold: false,
        };

        let cmd = wrap_command(&spec);

        assert!(cmd.contains("pnpm start:dev"));
        assert!(cmd.contains("tee \"/tmp/kws/w/backend/api.log\""));
        assert!(cmd.contains("/tmp/kws/w/backend/api.exit"));
        assert!(cmd.contains("exit \"$ec\""));
        assert!(!cmd.contains("exec \"$SHELL\""));
    }

    #[test]
    fn wraps_command_with_hold() {
        let env = HashMap::new();
        let spec = WrapSpec {
            run: "pnpm db:migrate",
            env: &env,
            cwd: Path::new("/home/caio/proj/backend"),
            log_file: Path::new("/tmp/kws/w/backend/migrate.log"),
            exit_file: Path::new("/tmp/kws/w/backend/migrate.exit"),
            hold: true,
        };

        let cmd = wrap_command(&spec);

        assert!(cmd.contains("exec \"$SHELL\""));
        assert!(!cmd.contains("exit \"$ec\""));
    }

    #[test]
    fn wraps_command_with_env() {
        let mut env = HashMap::new();
        env.insert("NODE_ENV".to_string(), "development".to_string());
        let spec = WrapSpec {
            run: "true",
            env: &env,
            cwd: Path::new("/tmp"),
            log_file: Path::new("/tmp/a.log"),
            exit_file: Path::new("/tmp/a.exit"),
            hold: false,
        };

        let cmd = wrap_command(&spec);

        assert!(cmd.contains("export NODE_ENV='development'"));
    }

    #[test]
    fn wraps_command_with_cwd() {
        let env = HashMap::new();
        let spec = WrapSpec {
            run: "pnpm dev",
            env: &env,
            cwd: Path::new("/home/caio/proj/vello-ai/apps/web"),
            log_file: Path::new("/tmp/a.log"),
            exit_file: Path::new("/tmp/a.exit"),
            hold: false,
        };

        let cmd = wrap_command(&spec);

        assert!(cmd.contains("if cd \"/home/caio/proj/vello-ai/apps/web\"; then"));
    }

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn paths_for_builds_predictable_layout() {
        let (log, exit) = paths_for(Path::new("/tmp/kws"), "financeiro", "backend", Some("api"));

        assert_eq!(log, PathBuf::from("/tmp/kws/financeiro/backend/api.log"));
        assert_eq!(exit, PathBuf::from("/tmp/kws/financeiro/backend/api.exit"));
    }

    #[test]
    fn paths_for_defaults_area_to_main() {
        let (log, _) = paths_for(Path::new("/tmp/kws"), "financeiro", "frontend", None);

        assert_eq!(log, PathBuf::from("/tmp/kws/financeiro/frontend/main.log"));
    }
}
