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
