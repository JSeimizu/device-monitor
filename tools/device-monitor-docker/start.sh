#!/bin/bash

cd /work

# Get IP address from environment variable or use default localhost
DEVICE_IP=${DEVICE_IP:-localhost}
USE_INTERNAL_MQTT=${USE_INTERNAL_MQTT:-false}
USE_INTERNAL_AZURITE=${USE_INTERNAL_AZURITE:-false}

if [ "x$USE_INTERNAL_MQTT" = "xtrue" ]; then
	# Allow anonymous connection

	echo "listener 1883 0.0.0.0" >> /etc/mosquitto/conf.d/local_network.conf
	echo "allow_anonymous true" >> /etc/mosquitto/conf.d/local_network.conf

	# Start mosquitto in daemon mode
	echo "Starting mosquitto MQTT broker..."
	sed -i 's%var/log/mosquitto%work%' /etc/mosquitto/mosquitto.conf
	mosquitto -c /etc/mosquitto/mosquitto.conf -d -v
else
	echo "Using external MQTT broker at ${DEVICE_IP}:1883"
fi

if [ "x$USE_INTERNAL_AZURITE" = "xtrue" ]; then
	# Start Azurite in daemon mode
	echo "Starting Azurite..."
	azurite --silent --location /tmp/azurite --debug /work/azurite.log \
		--blobHost 0.0.0.0 --blobPort 10000 \
		--queueHost 0.0.0.0 --queuePort 10001 \
		--tableHost 0.0.0.0 --tablePort 10002 &
else
	echo "Using external Azurite at ${DEVICE_IP}"
fi

if [ "x$USE_INTERNAL_MQTT" = "xtrue" ] || [ "x$USE_INTERNAL_AZURITE" = "xtrue" ]; then
	if [ "x$USE_INTERNAL_MQTT" = "xtrue" ]; then
		while ! ss -tuln | grep -q ':1883'; do
			echo "Waiting for MQTT broker to start..."
			sleep 1
		done
	fi

	if [ "x$USE_INTERNAL_AZURITE" = "xtrue" ]; then
		while ! ss -tuln | grep -q ':10000\|:10001\|:10002'; do
			echo "Waiting for Azurite to start..."
			sleep 1
		done
	fi
fi

echo ""
echo "Starting device-monitor..."

# Start device-monitor in foreground with connection URLs using specified IP
exec device-monitor -b ${DEVICE_IP}:1883 -a ${DEVICE_IP}:10000 -v -l /work/device-monitor.log
