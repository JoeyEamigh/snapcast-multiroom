# snapcast-multiroom

`snapcast-multiroom` is a Rust application to group multiple [snapcast](https://github.com/badaix/snapcast) clients into zones in a multiroom configuration. The idea derives from the [snapcast-autoconfig](https://github.com/ahayworth/snapcast-autoconfig) project, with a few tweaks such as using a event-based system rather than polling.

## Usage

The easiest way to get started is via Docker.

```bash
cp config.example.json config.json # Edit the config file to match your setup

docker run \
  -v $(pwd)/config.json:/config.json \
  ghcr.io/joeyeamigh/snapcast-multiroom:latest
```

## Configuration

The file [config.schema.json](config.schema.json) contains the schema for the configuration file and descriptions for each field. `snapcast-multiroom` will also accept the env var `CONFIG_PATH` to change the default location from `/config.json`.
