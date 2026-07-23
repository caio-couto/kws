use crate::{core::condition::Condition, error::KwsError};
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct ReadyWhen {
    #[serde(flatten)]
    pub condition: Condition,
    pub timeout: Option<String>,
}

impl ReadyWhen {
    pub(crate) fn validate(&self, tab_title: &str) -> Result<(), KwsError> {
        if let Condition::Http(url) = &self.condition
            && !url.starts_with("http://")
            && !url.starts_with("https://")
        {
            return Err(KwsError::ValidationError(format!(
                "URL inválida '{url}' na aba '{tab_title}': deve começar com http:// ou https://"
            )));
        }

        if let Condition::Delay(s) = &self.condition {
            Self::parse_duration(s)?;
        }

        if let Some(t) = &self.timeout {
            Self::parse_duration(t)?;
        }

        Ok(())
    }

    fn parse_duration(s: &str) -> Result<Duration, KwsError> {
        let s: &str = s.trim();

        let err =
            || KwsError::ValidationError(format!("duração inválida '{s}': use sufixo s, m ou h"));

        if let Some(n) = s.strip_suffix('h') {
            return n
                .trim()
                .parse::<u64>()
                .map(|n| Duration::from_secs(n * 3600))
                .map_err(|_| err());
        }

        if let Some(n) = s.strip_suffix('m') {
            return n
                .trim()
                .parse::<u64>()
                .map(|n| Duration::from_secs(n * 60))
                .map_err(|_| err());
        }

        if let Some(n) = s.strip_suffix('s') {
            return n
                .trim()
                .parse::<u64>()
                .map(Duration::from_secs)
                .map_err(|_| err());
        }

        Err(err())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::condition::Condition;

    fn rw(condition: Condition, timeout: Option<&str>) -> ReadyWhen {
        ReadyWhen {
            condition,
            timeout: timeout.map(str::to_string),
        }
    }

    #[test]
    fn parse_duration_seconds() {
        assert_eq!(ReadyWhen::parse_duration("30s").unwrap().as_secs(), 30);
        assert_eq!(ReadyWhen::parse_duration("1s").unwrap().as_secs(), 1);
    }

    #[test]
    fn parse_duration_minutes() {
        assert_eq!(ReadyWhen::parse_duration("5m").unwrap().as_secs(), 300);
    }

    #[test]
    fn parse_duration_hours() {
        assert_eq!(ReadyWhen::parse_duration("2h").unwrap().as_secs(), 7200);
    }

    #[test]
    fn parse_duration_invalid() {
        assert!(ReadyWhen::parse_duration("abc").is_err());
        assert!(ReadyWhen::parse_duration("30").is_err());
        assert!(ReadyWhen::parse_duration("").is_err());
        assert!(ReadyWhen::parse_duration("ms").is_err());
    }

    #[test]
    fn valid_http_url() {
        assert!(
            rw(Condition::Http("http://localhost:3000/health".into()), None)
                .validate("tab")
                .is_ok()
        );
        assert!(
            rw(Condition::Http("https://example.com".into()), None)
                .validate("tab")
                .is_ok()
        );
    }

    #[test]
    fn invalid_url_rejected() {
        assert!(
            rw(Condition::Http("localhost:3000".into()), None)
                .validate("tab")
                .is_err()
        );
        assert!(
            rw(Condition::Http("ftp://x.com".into()), None)
                .validate("tab")
                .is_err()
        );
    }

    #[test]
    fn valid_delay() {
        assert!(
            rw(Condition::Delay("10s".into()), None)
                .validate("tab")
                .is_ok()
        );
    }

    #[test]
    fn invalid_delay_rejected() {
        assert!(
            rw(Condition::Delay("10".into()), None)
                .validate("tab")
                .is_err()
        );
    }

    #[test]
    fn valid_timeout() {
        assert!(
            rw(Condition::Port(5432), Some("60s"))
                .validate("tab")
                .is_ok()
        );
    }

    #[test]
    fn invalid_timeout_rejected() {
        assert!(
            rw(Condition::Port(5432), Some("abc"))
                .validate("tab")
                .is_err()
        );
    }
}
