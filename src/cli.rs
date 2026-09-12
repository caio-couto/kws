use std::path::PathBuf;

pub const USAGE: &str = "\
kws: abre um workspace de terminal a partir de um arquivo de configuração

Uso:
    kws <nome> [dir] [--attach]

Argumentos:
    <nome>    nome do workspace, resolvido em ~/.config/kws/<nome>.toml
    [dir]     sobrescreve a 'base' do workspace ('.' usa o diretório atual)
    --attach  abre as abas numa janela do Konsole já existente, em vez de
              criar uma janela nova

Exemplos:
    kws financeiro
    kws financeiro .
    kws financeiro ~/proj/financeiro
    kws financeiro --attach
";

#[derive(Debug)]
pub struct Args {
    pub config_path: PathBuf,
    pub base_override: Option<PathBuf>,
    pub attach: bool,
}

pub fn parse(args: &[String]) -> Result<Args, String> {
    let attach = args.iter().any(|a| a == "--attach");
    let positional: Vec<&String> = args.iter().filter(|a| a.as_str() != "--attach").collect();

    if positional.is_empty() || positional.iter().any(|a| a.as_str() == "--help" || a.as_str() == "-h") {
        return Err(USAGE.to_string());
    }

    if positional.len() > 2 {
        return Err(USAGE.to_string());
    }

    let name = positional[0];
    let config_path = home_dir()
        .ok_or_else(|| "não foi possível determinar o diretório HOME".to_string())?
        .join(".config")
        .join("kws")
        .join(format!("{name}.toml"));

    let base_override = match positional.get(1) {
        None => None,
        Some(dir) if dir.as_str() == "." => {
            Some(std::env::current_dir().map_err(|e| format!("erro: {e}"))?)
        }
        Some(dir) => Some(crate::core::workspace::Workspace::expand_tilde(
            std::path::Path::new(dir.as_str()),
        )),
    };

    Ok(Args {
        config_path,
        base_override,
        attach,
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
    fn attach_flag_sets_attach_true() {
        let args = parse(&["financeiro".to_string(), "--attach".to_string()]).unwrap();

        assert!(args.attach);
        assert!(args.base_override.is_none());
    }

    #[test]
    fn without_attach_flag_defaults_to_false() {
        let args = parse(&["financeiro".to_string()]).unwrap();

        assert!(!args.attach);
    }

    #[test]
    fn attach_flag_works_alongside_dir_override() {
        let args = parse(&[
            "financeiro".to_string(),
            "/tmp".to_string(),
            "--attach".to_string(),
        ])
        .unwrap();

        assert!(args.attach);
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
