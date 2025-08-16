#!/bin/bash

# Get IP address from environment variable or use default localhost
DEVICE_IP=${DEVICE_IP:-localhost}

# Allow anonymous connection

echo "listener 1883 0.0.0.0" >> /etc/mosquitto/conf.d/local_network.conf
echo "allow_anonymous true" >> /etc/mosquitto/conf.d/local_network.conf

# Start mosquitto in daemon mode
echo "Starting mosquitto MQTT broker..."
mosquitto -c /etc/mosquitto/mosquitto.conf -d -v

# Start Azurite in daemon mode
echo "Starting Azurite..."
azurite --silent --location /tmp/azurite --debug /var/log/azurite_debug.log \
    --blobHost 0.0.0.0 --blobPort 10000 \
    --queueHost 0.0.0.0 --queuePort 10001 \
    --tableHost 0.0.0.0 --tablePort 10002 &

# Wait a moment for services to start
sleep 3

# Check if services are running
echo "Checking services..."
if ! pgrep mosquitto > /dev/null; then
    echo "Error: mosquitto failed to start"
    exit 1
fi

# Check if Azurite ports are listening instead of process name
if ! ss -tuln | grep -q ':10000\|:10001\|:10002'; then
    echo "Error: azurite failed to start"
    exit 1
fi

echo "Services started successfully!"
echo "MQTT broker: ${DEVICE_IP}:1883"
echo "Azurite blob: ${DEVICE_IP}:10000"
echo "Azurite queue: ${DEVICE_IP}:10001"
echo "Azurite table: ${DEVICE_IP}:10002"
echo ""
echo "Starting device-monitor..."

# Start device-monitor in foreground with connection URLs using specified IP
exec device-monitor -b ${DEVICE_IP}:1883 -a ${DEVICE_IP}:10000 -v -l /var/log/device-monitor.log
