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

  #[serde(default)]
  pub ports: Vec<String>,

  #[serde(default)]
  pub chown_dirs: Vec<String>,

  #[serde(default = "default_true")]
  pub user_mapping: bool,
}

fn default_true() -> bool {
  true
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

    for port in &self.ports {
      Self::validate_port_mapping(port)?;
    }

    Ok(())
  }

  fn validate_port_mapping(mapping: &str) -> Result<(), String> {
    // Strip optional protocol suffix (/tcp, /udp, /sctp)
    let without_proto = if let Some(idx) = mapping.rfind('/') {
      let proto = &mapping[idx + 1..];
      match proto {
        "tcp" | "udp" | "sctp" => &mapping[..idx],
        _ => {
          return Err(format!(
            "Invalid port protocol in '{}': must be tcp, udp, or sctp",
            mapping
          ));
        }
      }
    } else {
      mapping
    };

    // Must be "host_port:container_port" format
    let parts: Vec<&str> = without_proto.split(':').collect();
    if parts.len() != 2 {
      return Err(format!(
        "Invalid port mapping '{}': must be 'host_port:container_port' (e.g., '8080:80')",
        mapping
      ));
    }
    Self::parse_port(parts[0], mapping)?;
    Self::parse_port(parts[1], mapping)?;

    Ok(())
  }

  fn parse_port(s: &str, mapping: &str) -> Result<u16, String> {
    let port: u16 = s.parse().map_err(|_| {
      format!("Invalid port number '{}' in mapping '{}'", s, mapping)
    })?;
    if port == 0 {
      return Err(format!(
        "Port number must be between 1 and 65535 in mapping '{}'",
        mapping
      ));
    }
    Ok(port)
  }
}
