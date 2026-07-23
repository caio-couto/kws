use std::fmt;

use crate::core::driver_kind::DriverKind;

#[derive(Debug)]
pub enum KwsError {
    BaseDirectoryNotFound(std::path::PathBuf),
    ConfigFileNotFound(std::path::PathBuf),
    InvalidConfigFile {
        error: Box<toml::de::Error>,
        raw_content: String,
        file_path: std::path::PathBuf,
    },
    SystemIo(std::io::Error),
    UnsupportedDriver(DriverKind),
    ValidationError(String),
}

impl KwsError {
    fn find_line_col(content: &str, byte_index: usize) -> (usize, usize) {
        let mut line: usize = 1;
        let mut col: usize = 1;

        for (i, character) in content.char_indices() {
            if i >= byte_index {
                break;
            }

            if character == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }

        (line, col)
    }
}

impl fmt::Display for KwsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KwsError::SystemIo(error) => write!(f, "Erro de I/O {}", error),
            KwsError::ValidationError(error_message) => write!(f, "{}", error_message),
            KwsError::ConfigFileNotFound(path) => {
                write!(
                    f,
                    "Configuração de workspace não encontrada em {}",
                    path.to_str().unwrap_or("")
                )
            }
            KwsError::InvalidConfigFile {
                error,
                raw_content,
                file_path,
            } => {
                if let Some(span) = error.span() {
                    let (line, col) = KwsError::find_line_col(raw_content, span.start);

                    write!(
                        f,
                        "Erro de sintaxe TOML no arquivo de configurações {:?}:{}:{}\nDetalhe: {}",
                        file_path,
                        line,
                        col,
                        error.message()
                    )
                } else {
                    write!(
                        f,
                        "Erro no ficheiro de configuração {:?}: {}",
                        file_path, error
                    )
                }
            }
            KwsError::BaseDirectoryNotFound(path) => {
                write!(
                    f,
                    "Não foi possível rastrear o diretório base: {}",
                    path.to_str().unwrap_or("")
                )
            }
            KwsError::UnsupportedDriver(driver) => {
                write!(f, "O driver {} não é suportado pelo sistema", driver)
            }
        }
    }
}
