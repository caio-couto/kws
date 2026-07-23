use serde::{Deserialize, Deserializer, de::Error};

#[derive(Debug, Clone)]
pub enum Weight {
    Percent(u8),
    Factor(f64),
}

impl<'de> Deserialize<'de> for Weight {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Str(String),
            Float(f64),
        }

        match Raw::deserialize(deserializer)? {
            Raw::Float(f) => {
                if f <= 0.0 || f >= 1.0 {
                    return Err(D::Error::custom(format!(
                        "fator decimal deve estar entre 0.0 e 1.0, recebido: '{f}'"
                    )));
                }

                Ok(Self::Factor(f))
            }

            Raw::Str(s) => {
                let s: &str = s.trim();

                let num_str: &str = s.strip_suffix('%').ok_or_else(|| {
                    D::Error::custom(format!("proporção inválida '{s}': use '60%' ou 0.6"))
                })?;

                let n: u8 = num_str.parse().map_err(|_| {
                    D::Error::custom(format!("'{num_str}' não é um número inteiro válido"))
                })?;

                if n > 100 {
                    return Err(D::Error::custom(format!(
                        "porcentagem não pode exceder 100%, recebido: '{n}%'"
                    )));
                }

                Ok(Self::Percent(n))
            }
        }
    }
}
