#!/usr/bin/env bash

set -euo pipefail

mkdir -p /work
cd /work

# Environment
DEVICE_IP=${DEVICE_IP:-localhost}
USE_INTERNAL_MQTT=${USE_INTERNAL_MQTT:-false}
USE_INTERNAL_AZURITE=${USE_INTERNAL_AZURITE:-false}

# Helper to wait for TCP port
wait_for_port() {
  local host="$1"; local port="$2"; local name="$3"; local attempts=0
  until bash -c ">/dev/tcp/${host}/${port}" >/dev/null 2>&1; do
    echo "Waiting for ${name} on ${host}:${port}..."
    sleep 1
    attempts=$((attempts+1))
    if [ "${attempts}" -gt 60 ]; then
      echo "ERROR: ${name} did not start in time"
      exit 1
    fi
  done
}

# Start internal MQTT broker if requested
if [ "${USE_INTERNAL_MQTT}" = "true" ]; then
  echo "Configuring internal MQTT broker..."
  cat > /etc/mosquitto/conf.d/99-local.conf << 'EOF'
listener 1883 0.0.0.0
allow_anonymous true
log_dest stdout
EOF
  echo "Starting mosquitto MQTT broker..."
  mosquitto -c /etc/mosquitto/mosquitto.conf -d -v
else
  echo "Using external MQTT broker at ${DEVICE_IP}:1883"
fi

# Start internal Azurite if requested
if [ "${USE_INTERNAL_AZURITE}" = "true" ]; then
  echo "Starting Azurite..."
  azurite --silent --location /tmp/azurite --debug /work/azurite.log \
    --blobHost 0.0.0.0 --blobPort 10000 \
    --queueHost 0.0.0.0 --queuePort 10001 \
    --tableHost 0.0.0.0 --tablePort 10002 &
else
  echo "Using external Azurite at ${DEVICE_IP}"
fi

# Wait for internal services if enabled
if [ "${USE_INTERNAL_MQTT}" = "true" ]; then
  wait_for_port 127.0.0.1 1883 MQTT
fi
if [ "${USE_INTERNAL_AZURITE}" = "true" ]; then
  wait_for_port 127.0.0.1 10000 Azurite-Blob
fi

# Select hosts for device-monitor endpoints
BROKER_HOST="$DEVICE_IP"
AZURITE_HOST="$DEVICE_IP"
if [ "${USE_INTERNAL_MQTT}" = "true" ]; then
  BROKER_HOST="127.0.0.1"
fi
if [ "${USE_INTERNAL_AZURITE}" = "true" ]; then
  AZURITE_HOST="127.0.0.1"
fi

echo ""
echo "Starting device-monitor..."

# Ensure /work is writable by non-root user
chown device:device /work || true

# Start device-monitor in foreground with connection URLs
exec su -s /bin/sh -c "exec device-monitor -b ${BROKER_HOST}:1883 -a ${AZURITE_HOST}:10000 -v -l /work/device-monitor.log" device
