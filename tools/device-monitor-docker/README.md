# Device Monitor Docker Image

This manual describes how to build and run the device-monitor Docker image.

## How to Build
You can retrieve the prebuilt Docker image as follows:
```
docker pull ghcr.io/jseimizu/device-monitor:<TAG>
```
If you want to build the Docker image yourself, you can do so as follows:

```bash
cd tools/device-monitor-docker
# Optionally pin the bundled device-monitor version and checksum
docker build -f Dockerfile \
  [--build-arg DEVICE_MONITOR_VERSION=<VERSION>] \
  [--build-arg DEVICE_MONITOR_SHA256=<SHA256>] \
  -t device-monitor:<TAG> .
```

Note that by default the latest version of the `device-monitor` binary will be downloaded from the release page and used. If `DEVICE_MONITOR_VERSION` is provided at build time, that version is used instead. When `DEVICE_MONITOR_SHA256` is provided, the download is verified against the checksum.

## How to Run
```bash
docker run --rm -it \
           -v <WORK_DIR>:/work \
           -e DEVICE_IP=<DEVICE_IP> \
           [-e USE_INTERNAL_MQTT=true -p 1883:1883] \
           [-e USE_INTERNAL_AZURITE=true -p 10000:10000 -p 10001:10001 -p 10002:10002] \
           device-monitor:<TAG>
```
Where:
* `<WORK_DIR>` is the directory where device-monitor runs and where logs will be stored.
* `<DEVICE_IP>` is the local IP address that the edge device should connect to.
* If `USE_INTERNAL_MQTT` is set to `true`, the internal MQTT broker will be started and port 1883 should be published. Inside the container, device-monitor connects to `127.0.0.1:1883`. If not set, device-monitor connects to an external MQTT broker at `DEVICE_IP:1883`.
* If `USE_INTERNAL_AZURITE` is set to `true`, the internal Azurite storage emulator will be started and blob port 10000 (and queue/table if needed) should be published. device-monitor always uses `DEVICE_IP:10000` for Azurite so that generated SAS URLs are reachable by edge devices on the local network.
* `<TAG>` is the tag of the device-monitor Docker image.

Notes:
- Base image is pinned (`ubuntu:24.04`) and Azurite is version-pinned for reproducibility.
- The container creates `/work` if not present. `device-monitor` runs as a non-root user for improved security.
- A `HEALTHCHECK` validates internal services (when enabled) and `device-monitor` liveness.
