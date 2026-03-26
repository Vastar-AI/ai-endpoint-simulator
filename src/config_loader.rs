use serde::Deserialize;

#[derive(Deserialize)]
pub struct DatabaseConfig {
    pub username: String,
    pub password: String,
    pub url: String,
}

#[derive(Deserialize)]
pub struct BindingConfig {
    pub port: u16,
    pub host: String,
}

#[derive(Deserialize)]
pub struct RedisConfig {
    pub url: String,
    #[serde(default = "default_redis_prefix")]
    pub prefix: String,
}

fn default_redis_prefix() -> String {
    "ai_simulator".to_string()
}

#[derive(Deserialize)]
pub struct Config {
    pub source: String,
    pub database: DatabaseConfig,
    pub binding: BindingConfig,
    pub tracking: TrackingConfig,
    pub log_level: String,
    pub channel_capacity: usize,
    pub semaphore_limit: usize,
    pub workers: usize,
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl: u64,
    pub redis: RedisConfig,
}

#[derive(Deserialize)]
pub struct TrackingConfig {
    pub enabled: bool,
}

fn default_cache_ttl() -> u64 {
    60 // Default cache TTL: 60 seconds
}

impl Config {
    pub fn load() -> Self {
        let config_str = std::fs::read_to_string("config.yml").expect("Failed to read config file");
        serde_yaml::from_str(&config_str).expect("Failed to parse config file")
    }
}
