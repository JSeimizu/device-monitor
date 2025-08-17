#!/usr/bin/env bash
set -euo pipefail

# Check MQTT if internal broker is enabled
if [ "${USE_INTERNAL_MQTT:-false}" = "true" ]; then
  if ! bash -c ">/dev/tcp/127.0.0.1/1883" >/dev/null 2>&1; then
    echo "MQTT not responding on 127.0.0.1:1883"
    exit 1
  fi
fi

# Check Azurite (Blob) if internal azurite is enabled
if [ "${USE_INTERNAL_AZURITE:-false}" = "true" ]; then
  if ! bash -c ">/dev/tcp/127.0.0.1/10000" >/dev/null 2>&1; then
    echo "Azurite Blob not responding on 127.0.0.1:10000"
    exit 1
  fi
fi

# Ensure device-monitor process is running
if ! pgrep -x device-monitor >/dev/null 2>&1; then
  echo "device-monitor is not running"
  exit 1
fi

exit 0

