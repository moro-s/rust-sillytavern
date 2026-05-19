use anyhow::Context;
use serde::Deserialize;

const CONFIG_PATH: &str = "config.toml";

#[derive(Debug, Clone, Deserialize)]
pub struct LlmConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub llm: LlmConfig,
}

pub fn load() -> anyhow::Result<Config> {
    let content = std::fs::read_to_string(CONFIG_PATH)
        .with_context(|| format!("Cannot read config file: {CONFIG_PATH}"))?;
    let cfg: Config = toml::from_str(&content)
        .with_context(|| format!("Invalid TOML in {CONFIG_PATH}"))?;
    Ok(cfg)
}
