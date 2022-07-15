use config::ConfigError;
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u32,
    pub database_url: String,
}

#[derive(Serialize, Deserialize)]
pub struct ContractConfig {
    pub factory: String,
    pub router: String,
    pub rpc: String,
    pub chain_id: u32,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub contract: ContractConfig,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut cfg = config::Config::new();
        cfg.merge(config::Environment::new())?;
        cfg.try_into()
    }
}
