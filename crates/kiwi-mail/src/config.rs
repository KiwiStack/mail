use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

const DEFAULT_CONFIG_PATH: &str = "/etc/kiwi/config.toml";

fn default_listen_addr() -> String {
    "0.0.0.0:8443".to_string()
}

fn default_upstream_addr() -> String {
    "http://127.0.0.1:8080".to_string()
}

fn default_upstream_bin() -> PathBuf {
    PathBuf::from("/opt/upstream/bin/stalwart-mail")
}

fn default_upstream_config() -> PathBuf {
    PathBuf::from("/opt/upstream/etc/config.toml")
}

fn default_health_check_interval() -> HumanDuration {
    HumanDuration(Duration::from_secs(5))
}

fn default_health_check_timeout() -> HumanDuration {
    HumanDuration(Duration::from_secs(30))
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_admin_user() -> String {
    "admin".to_string()
}

fn default_admin_pass() -> String {
    "changeme".to_string()
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,

    #[serde(default = "default_upstream_addr")]
    pub upstream_addr: String,

    #[serde(default = "default_upstream_bin")]
    pub upstream_bin: PathBuf,

    #[serde(default = "default_upstream_config")]
    pub upstream_config: PathBuf,

    #[serde(default = "default_health_check_interval")]
    pub health_check_interval: HumanDuration,

    #[serde(default = "default_health_check_timeout")]
    pub health_check_timeout: HumanDuration,

    #[serde(default = "default_log_level")]
    pub log_level: String,

    #[serde(default = "default_admin_user")]
    pub admin_user: String,

    #[serde(default = "default_admin_pass")]
    pub admin_pass: String,
}

#[derive(Debug)]
pub struct HumanDuration(pub Duration);

impl<'de> Deserialize<'de> for HumanDuration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        parse_duration(&s)
            .map(HumanDuration)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid duration: {s}")))
    }
}

fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if let Some(val) = s.strip_suffix('s') {
        val.trim().parse::<u64>().ok().map(Duration::from_secs)
    } else if let Some(val) = s.strip_suffix('m') {
        val.trim()
            .parse::<u64>()
            .ok()
            .map(|m| Duration::from_secs(m * 60))
    } else {
        s.parse::<u64>().ok().map(Duration::from_secs)
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let path = std::env::var("KIWI_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_CONFIG_PATH));
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_config() {
        let toml = r#"
            listen_addr = "0.0.0.0:9443"
            upstream_addr = "http://127.0.0.1:9090"
            upstream_bin = "/usr/bin/stalwart"
            upstream_config = "/etc/stalwart.toml"
            health_check_interval = "10s"
            health_check_timeout = "1m"
            log_level = "debug"
            admin_user = "root"
            admin_pass = "secret"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.listen_addr, "0.0.0.0:9443");
        assert_eq!(config.upstream_addr, "http://127.0.0.1:9090");
        assert_eq!(config.health_check_interval.0, Duration::from_secs(10));
        assert_eq!(config.health_check_timeout.0, Duration::from_secs(60));
        assert_eq!(config.log_level, "debug");
        assert_eq!(config.admin_user, "root");
        assert_eq!(config.admin_pass, "secret");
    }

    #[test]
    fn parse_defaults() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.listen_addr, "0.0.0.0:8443");
        assert_eq!(config.upstream_addr, "http://127.0.0.1:8080");
        assert_eq!(config.health_check_interval.0, Duration::from_secs(5));
        assert_eq!(config.health_check_timeout.0, Duration::from_secs(30));
    }
}
