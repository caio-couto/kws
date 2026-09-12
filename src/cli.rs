use std::path::PathBuf;

pub const USAGE: &str = "\
kws: abre um workspace de terminal a partir de um arquivo de configuração

Uso:
    kws <nome> [dir]

Argumentos:
    <nome>    nome do workspace, resolvido em ~/.config/kws/<nome>.toml
    [dir]     sobrescreve a 'base' do workspace ('.' usa o diretório atual)

Exemplos:
    kws financeiro
    kws financeiro .
    kws financeiro ~/proj/financeiro
";

#[derive(Debug)]
pub struct Args {
    pub config_path: PathBuf,
    pub base_override: Option<PathBuf>,
}

pub fn parse(args: &[String]) -> Result<Args, String> {
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        return Err(USAGE.to_string());
    }

    if args.len() > 2 {
        return Err(USAGE.to_string());
    }

    let name = &args[0];
    let config_path = home_dir()
        .ok_or_else(|| "não foi possível determinar o diretório HOME".to_string())?
        .join(".config")
        .join("kws")
        .join(format!("{name}.toml"));

    let base_override = match args.get(1) {
        None => None,
        Some(dir) if dir == "." => Some(std::env::current_dir().map_err(|e| format!("erro: {e}"))?),
        Some(dir) => Some(crate::core::workspace::Workspace::expand_tilde(
            std::path::Path::new(dir),
        )),
    };

    Ok(Args {
        config_path,
        base_override,
    })
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args_returns_usage_error() {
        let err = parse(&[]).unwrap_err();

        assert!(err.contains("Uso:"));
    }

    #[test]
    fn help_flag_returns_usage() {
        let err = parse(&["--help".to_string()]).unwrap_err();

        assert!(err.contains("Uso:"));
    }

    #[test]
    fn name_only_resolves_config_path_under_home_config_kws() {
        let args = parse(&["financeiro".to_string()]).unwrap();
        let expected_suffix = std::path::Path::new(".config/kws/financeiro.toml");

        assert!(args.config_path.ends_with(expected_suffix));
        assert!(args.base_override.is_none());
    }

    #[test]
    fn name_with_dot_sets_base_override_to_current_dir() {
        let args = parse(&["financeiro".to_string(), ".".to_string()]).unwrap();

        assert_eq!(args.base_override, Some(std::env::current_dir().unwrap()));
    }

    #[test]
    fn name_with_explicit_dir_sets_base_override() {
        let args = parse(&["financeiro".to_string(), "/tmp".to_string()]).unwrap();

        assert_eq!(args.base_override, Some(PathBuf::from("/tmp")));
    }

    #[test]
    fn too_many_args_returns_usage_error() {
        let err = parse(&[
            "financeiro".to_string(),
            ".".to_string(),
            "extra".to_string(),
        ])
        .unwrap_err();

        assert!(err.contains("Uso:"));
    }
}
