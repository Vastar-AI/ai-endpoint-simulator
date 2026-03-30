use serde::Deserialize;

#[derive(Deserialize)]
pub struct BindingConfig {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_host")]
    pub host: String,
}

fn default_port() -> u16 { 4545 }
fn default_host() -> String { "0.0.0.0".to_string() }

#[derive(Deserialize)]
pub struct Config {
    #[serde(default)]
    pub binding: BindingConfig,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_channel_capacity")]
    pub channel_capacity: usize,
    #[serde(default = "default_semaphore_limit")]
    pub semaphore_limit: usize,
    #[serde(default = "default_workers")]
    pub workers: usize,
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl: u64,
}

impl Default for BindingConfig {
    fn default() -> Self {
        Self { port: default_port(), host: default_host() }
    }
}

fn default_log_level() -> String { "info".to_string() }
fn default_channel_capacity() -> usize { 1000 }
fn default_semaphore_limit() -> usize { 10000 }
fn default_workers() -> usize { 4 }
fn default_cache_ttl() -> u64 { 60 }

const EMBEDDED_CONFIG: &str = r#"
binding:
  port: 4545
  host: 0.0.0.0
log_level: info
channel_capacity: 1000
semaphore_limit: 10000
workers: 4
cache_ttl: 60
"#;

impl Config {
    pub fn load() -> Self {
        let config_str = std::fs::read_to_string("config.yml")
            .unwrap_or_else(|_| EMBEDDED_CONFIG.to_string());
        serde_yaml::from_str(&config_str).unwrap_or_else(|_| {
            // If config parse fails, use defaults
            serde_yaml::from_str(EMBEDDED_CONFIG).expect("Embedded config must be valid")
        })
    }
}
