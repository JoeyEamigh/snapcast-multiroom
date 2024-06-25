#!/bin/bash

VERSION=$(awk 'sub(/^[[:space:]]*version[[:space:]]*=[[:space:]]*/, "") {
    sub(/^"/, ""); sub(/".*$/, "")
    print
}' Cargo.toml)

docker build \
  --build-arg VERSION="$VERSION" \
  -t ghcr.io/joeyeamigh/snapcast-multiroom:v"$VERSION" \
  -t ghcr.io/joeyeamigh/snapcast-multiroom:latest \
  --push \
  .
