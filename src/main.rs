#![feature(let_chains)]

use std::collections::HashMap;

use config::MultiroomConfig;
use snapcast_control::{stream::StreamStatus, ClientError, Notification, SnapcastConnection, ValidMessage};

mod config;
mod monitoring;

#[tokio::main]
async fn main() {
  monitoring::init_logger();
  let config = config::init_config();

  let mut client =
    SnapcastConnection::open(config.server_address.parse().expect("could not parse socket address")).await;

  let group_mapping = initial_setup(&config, &mut client)
    .await
    .expect("could not perform initial setup");

  tracing::info!("ready! listening for updates.");

  loop {
    tokio::select! {
      Some(message) = client.recv() => {
        if let Ok(response) = message {
          tracing::trace!("response: {:#?}", response);

          if let ValidMessage::Notification { method, .. } = response {
            if let Err(error) = handle_notification(&config, &group_mapping, &mut client, *method).await {
              tracing::error!("could not handle notification: {:#?}", error);
            }
          }
        } else if let Err(err) = message {
          tracing::error!("decoder error: {:#?}", err);
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
  method: Notification,
) -> Result<(), ClientError> {
  if let Notification::StreamOnUpdate { params } = method {
    if let Some(stream_config) = config.zones.get(&params.id) {
      match params.stream.status {
        StreamStatus::Playing => {
          tracing::debug!("handling stream playing update for: {}", params.id);
          let state = client.state.clone();

          for group in &stream_config.groups {
            if let Some(group_id) = group_mapping.get(group)
              && let Some(state_group) = state.groups.get(group_id)
              && state_group.stream_id != params.id
            {
              tracing::info!("setting group {} to stream: {}", group, params.id);

              client.group_set_stream(group_id.clone(), params.id.clone()).await?;
            } else {
              tracing::debug!("no need to update group {} for stream: {}", group, params.id);
            }
          }
        }
        _ => tracing::debug!("ignoring stream update for: {}", params.id),
      }
    } else {
      tracing::debug!("no config for stream: {}", params.id);
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

  client.recv().await.expect("could not read from stream")?;

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
