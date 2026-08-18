use std::collections::HashMap;

use config::{MultiroomConfig, VolumeControl};
use snapcast_control::{
  ClientError, Notification, SnapcastConnection, ValidMessage,
  client::ClientVolume,
  stream::{StreamPlaybackStatus, StreamStatus},
};

mod config;
mod monitoring;

type Volumes = HashMap<String, usize>;

#[tokio::main]
async fn main() {
  monitoring::init_logger();
  let config = config::init_config();

  let mut client = SnapcastConnection::open(config.server_address.parse().expect("could not parse socket address"))
    .await
    .expect("could not connect to snapcast server");

  let group_mapping = initial_setup(&config, &mut client)
    .await
    .expect("could not perform initial setup");

  tracing::info!("ready! listening for updates.");

  let mut volumes = Volumes::new();

  loop {
    tokio::select! {
      Some(messages) = client.recv() => {
        for message in messages {
          match message {
            Ok(response) => {
              tracing::trace!("response: {:#?}", response);

              if let ValidMessage::Notification { method, .. } = response
                && let Err(error) = handle_notification(&config, &group_mapping, &mut client, &mut volumes, *method).await
              {
                tracing::error!("could not handle notification: {:#?}", error);
              }
            }
            Err(err) => tracing::error!("decoder error: {:#?}", err),
          }
        }
      },
      _ = monitoring::wait_for_signal() => break,
    }
  }
}

async fn handle_notification(
  config: &MultiroomConfig,
  group_mapping: &HashMap<String, String>,
  client: &mut SnapcastConnection,
  volumes: &mut Volumes,
  method: Notification,
) -> Result<(), ClientError> {
  match method {
    Notification::StreamOnUpdate { params } => {
      if params.stream.status == StreamStatus::Playing {
        activate_zone(config, group_mapping, client, &params.id).await?;
      } else {
        tracing::debug!("ignoring stream update for: {}", params.id);
      }
    }

    Notification::StreamOnProperties { params } => {
      if params.properties.playback_status == Some(StreamPlaybackStatus::Playing) {
        activate_zone(config, group_mapping, client, &params.id).await?;
      }

      if let Some(volume) = params.properties.volume
        && volumes.get(&params.id) != Some(&volume)
      {
        volumes.insert(params.id.clone(), volume);
        apply_zone_volume(config, group_mapping, client, &params.id, volume).await?;
      }
    }

    _ => {}
  }

  Ok(())
}

async fn activate_zone(
  config: &MultiroomConfig,
  group_mapping: &HashMap<String, String>,
  client: &mut SnapcastConnection,
  zone_id: &str,
) -> Result<(), ClientError> {
  let Some(zone) = config.zones.get(zone_id) else {
    tracing::debug!("no config for stream: {zone_id}");
    return Ok(());
  };

  let state = client.state.clone();

  for group in &zone.groups {
    if let Some(group_id) = group_mapping.get(group)
      && let Some(state_group) = state.groups.get(group_id)
      && state_group.stream_id != zone_id
    {
      tracing::info!("setting group {group} to stream: {zone_id}");

      client.group_set_stream(group_id.clone(), zone_id.to_string()).await?;
    } else {
      tracing::debug!("no need to update group {group} for stream: {zone_id}");
    }
  }

  Ok(())
}

async fn apply_zone_volume(
  config: &MultiroomConfig,
  group_mapping: &HashMap<String, String>,
  client: &mut SnapcastConnection,
  zone_id: &str,
  volume: usize,
) -> Result<(), ClientError> {
  let Some(zone) = config.zones.get(zone_id) else {
    return Ok(());
  };

  if zone.volume_control == VolumeControl::Source {
    tracing::trace!("zone {zone_id} attenuates at the source");
    return Ok(());
  }

  let state = client.state.clone();
  let percent = volume.min(100);

  for group in &zone.groups {
    let Some(group_id) = group_mapping.get(group) else {
      continue;
    };
    let Some(state_group) = state.groups.get(group_id) else {
      continue;
    };

    if state_group.stream_id != zone_id {
      tracing::debug!("group {group} is not on {zone_id}, leaving its volume alone");
      continue;
    }

    for device in &state_group.clients {
      let muted = state.clients.get(device).is_some_and(|c| c.config.volume.muted);

      tracing::info!("setting {device} to {percent}% for zone {zone_id}");

      client
        .client_set_volume(device.clone(), ClientVolume { muted, percent })
        .await?;
    }
  }

  Ok(())
}

async fn initial_setup(
  config: &MultiroomConfig,
  client: &mut SnapcastConnection,
) -> Result<HashMap<String, String>, ClientError> {
  tracing::info!("starting initial sync...");
  let state = client.state.clone();

  client.server_get_status().await?;

  for message in client.recv().await.unwrap_or_default() {
    message?;
  }

  tracing::info!("performing initial setup...");

  for stream in &state.streams {
    if !config.zones.contains_key(stream.key()) {
      tracing::debug!("deleting stream: {}", stream.key());

      client.stream_remove_stream(stream.key().clone()).await?;
    }
  }

  for (id, zone) in &config.zones {
    if !state.streams.contains_key(id) {
      tracing::debug!("adding stream for zone: {}", zone.id);

      client.stream_add_stream(zone.stream.clone()).await?;
    }
  }

  for (id, device) in &config.devices {
    if let Some(preferred_name) = &device.preferred_name
      && let Some(device) = state.clients.get(id)
      && device.config.name != *preferred_name
    {
      tracing::debug!("setting preferred name for: {}", preferred_name);

      client.client_set_name(id.clone(), preferred_name.clone()).await?;
    }
  }

  let mut groups_map = HashMap::new();
  for group in &config.groups {
    if let Some(likely_group) = state.groups.iter().find(|g| g.clients.is_subset(&group.devices)) {
      groups_map.insert(group.name.clone(), likely_group.id.clone());

      if likely_group.name != group.name {
        tracing::debug!("setting group name: {}", group.name);
        client
          .group_set_name(likely_group.id.clone(), group.name.clone())
          .await?;
      }

      if likely_group.clients != group.devices {
        tracing::debug!("setting group clients: {:?}", group.devices);

        client
          .group_set_clients(likely_group.id.clone(), group.devices.iter().cloned().collect())
          .await?;
      }

      if let Some((default_zone, _)) = config
        .zones
        .iter()
        .find(|(_, z)| z.groups.len() == 1 && z.groups.contains(&group.name))
        && likely_group.stream_id != *default_zone
      {
        tracing::debug!("setting default stream for group {}: {}", &group.name, default_zone);

        client
          .group_set_stream(likely_group.id.clone(), default_zone.clone())
          .await?;
      }
    }
  }

  tracing::info!("initial setup complete, resyncing...");

  client.server_get_status().await?;

  Ok(groups_map)
}
