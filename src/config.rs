use serde::Deserialize;
use std::path::Path;
use std::fs;

const CONFIG_FILE: &str = ".devspace.toml";

#[derive(Debug, Deserialize)]
pub struct Config {
  pub shell: Option<String>,
  pub init: Option<String>,
  pub image: String,
  pub name: String,

  #[serde(default)]
  pub privileged: bool,

  #[serde(default)]
  pub gui: bool,
}

impl Config {
  pub fn load() -> Result<Self, String> {
    let path = Path::new(CONFIG_FILE);

    if !path.exists() {
      return Err(format!("{} not found in current directory", CONFIG_FILE));
    }

    let content = fs::read_to_string(path)
      .map_err(|e| format!("Failed to read {}: {}", CONFIG_FILE, e))?;

    let config: Self = toml::from_str(&content)
      .map_err(|e| format!("Failed to parse {}: {}", CONFIG_FILE, e))?;

    config.validate()?;
    Ok(config)
  }

  fn validate(&self) -> Result<(), String> {
    if self.name.is_empty() {
      return Err("Container name cannot be empty".to_string());
    }

    if !self
      .name
      .chars()
      .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
      return Err(
                "Container name must contain only alphanumeric characters, hyphens, or underscores"
                    .to_string(),
            );
    }

    if self.image.is_empty() {
      return Err("Image cannot be empty".to_string());
    }

    Ok(())
  }
}
