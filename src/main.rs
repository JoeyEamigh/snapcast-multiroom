#![feature(let_chains)]

use std::collections::HashMap;

use config::MultiroomConfig;
use snapcast_control::{ClientError, Notification, SnapcastConnection, ValidMessage};

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
      _ = tokio::signal::ctrl_c() => {
        tracing::info!("ctrl-c received, shutting down");
        break;
      }
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
      tracing::info!("setting groups to stream: {}", params.id);

      for group in &stream_config.groups {
        if let Some(group_id) = group_mapping.get(group) {
          client.group_set_stream(group_id.clone(), params.id.clone()).await?;
        }
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
    }
  }

  tracing::info!("initial setup complete, resyncing...");

  client.server_get_status().await?;

  Ok(groups_map)
}
