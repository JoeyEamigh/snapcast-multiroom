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
#[serde(rename_all = "camelCase")]
pub struct MultiroomConfigZone {
  pub id: String,
  pub stream: String,
  pub groups: HashSet<String>,
  #[serde(default)]
  pub volume_control: VolumeControl,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum VolumeControl {
  #[default]
  Clients,
  Source,
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
  fn zones_keep_pushing_client_volume_unless_told_otherwise() {
    let librespot = r#"{"id":"A","stream":"librespot:///usr/bin/librespot","groups":["g"]}"#;
    let zone: MultiroomConfigZone = serde_json::from_str(librespot).expect("could not deserialize zone");
    assert_eq!(zone.volume_control, VolumeControl::Clients);

    let soloist = r#"{"id":"A","stream":"pipe:///run/snapfifo/a","groups":["g"],"volumeControl":"source"}"#;
    let zone: MultiroomConfigZone = serde_json::from_str(soloist).expect("could not deserialize zone");
    assert_eq!(zone.volume_control, VolumeControl::Source);
  }

  #[test]
  fn test_deserialize_config() {
    let config_json = include_str!("../config.example.json");
    let _: MultiroomConfigJson = serde_json::from_str(config_json).expect("could not deserialize config");
  }

  #[test]
  fn test_deserialize_soloist_config() {
    let config_json = include_str!("../config.soloist.example.json");
    let config: MultiroomConfigJson =
      serde_json::from_str(config_json).expect("could not deserialize soloist config");

    assert!(
      config.zones.iter().all(|zone| zone.volume_control == VolumeControl::Source),
      "soloist zones must let the source own the master volume"
    );
  }
}
