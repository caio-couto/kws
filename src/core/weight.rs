use serde::{Deserialize, Deserializer, de::Error};

#[derive(Debug, Clone)]
pub enum Weight {
    Percent(u8),
    // Lido só quando o driver passar a aplicar proporções nos splits do Konsole.
    #[allow(dead_code)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize)]
    struct W {
        w: Weight,
    }

    fn parse(s: &str) -> Result<Weight, toml::de::Error> {
        toml::from_str::<W>(&format!("w = {s}")).map(|w| w.w)
    }

    #[test]
    fn valid_percent() {
        assert!(matches!(parse("\"60%\"").unwrap(), Weight::Percent(60)));
        assert!(matches!(parse("\"0%\"").unwrap(), Weight::Percent(0)));
        assert!(matches!(parse("\"100%\"").unwrap(), Weight::Percent(100)));
    }

    #[test]
    fn percent_over_100_rejected() {
        assert!(parse("\"101%\"").is_err());
    }

    #[test]
    fn percent_without_suffix_rejected() {
        assert!(parse("\"60\"").is_err());
    }

    #[test]
    fn valid_factor() {
        assert!(matches!(parse("0.4").unwrap(), Weight::Factor(f) if f == 0.4));
        assert!(matches!(parse("0.1").unwrap(), Weight::Factor(_)));
        assert!(matches!(parse("0.999").unwrap(), Weight::Factor(_)));
    }

    #[test]
    fn factor_out_of_range_rejected() {
        assert!(parse("0.0").is_err());
        assert!(parse("1.0").is_err());
        assert!(parse("1.5").is_err());
        assert!(parse("-0.5").is_err());
    }
}
