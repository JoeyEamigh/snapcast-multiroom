use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct MultiroomConfigJson {
  pub server_address: String,
  pub devices: Vec<MultiroomConfigDevice>,
  pub groups: Vec<MultiroomConfigGroup>,
  pub zones: Vec<MultiroomConfigZone>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MultiroomConfig {
  pub server_address: String,
  pub devices: HashMap<String, MultiroomConfigDevice>,
  pub groups: Vec<MultiroomConfigGroup>,
  pub zones: HashMap<String, MultiroomConfigZone>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MultiroomConfigDevice {
  pub id: String,
  pub preferred_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MultiroomConfigGroup {
  pub name: String,
  pub devices: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct MultiroomConfigZone {
  pub id: String,
  pub stream: String,
  pub groups: HashSet<String>,
}

pub fn init_config() -> MultiroomConfig {
  #[cfg(debug_assertions)]
  let config_path = std::env::var("CONFIG_PATH").unwrap_or("config.json".to_string());
  #[cfg(not(debug_assertions))]
  let config_path = std::env::var("CONFIG_PATH").unwrap_or("/config.json".to_string());

  tracing::debug!("loading config from {}", config_path);

  let file = std::fs::File::open(config_path).expect("could not open config file");
  let reader = std::io::BufReader::new(file);

  let config_json: MultiroomConfigJson = serde_json::from_reader(reader).expect("could not parse config file");
  tracing::trace!("loaded config {:#?}", config_json);

  tracing::debug!("converting config to internal format...");
  let config = MultiroomConfig {
    server_address: config_json.server_address,
    devices: HashMap::from_iter(
      config_json
        .devices
        .into_iter()
        .map(|device| (device.id.clone(), device)),
    ),
    groups: config_json.groups,
    zones: HashMap::from_iter(config_json.zones.into_iter().map(|zone| (zone.id.clone(), zone))),
  };

  tracing::trace!("converted config {:#?}", config);

  tracing::info!("successfully loaded config!");

  config
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_deserialize_config() {
    let config_json = include_str!("../config.example.json");
    let _: MultiroomConfig = serde_json::from_str(config_json).expect("could not deserialize config");
  }
}
