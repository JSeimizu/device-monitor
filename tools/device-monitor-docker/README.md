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
docker build -f Dockerfile -t device-monitor:<TAG> .
```

Note that the latest version of the `device-monitor` binary will be downloaded from the release page and used.

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
* If `USE_INTERNAL_MQTT` is set to `true`, the internal MQTT broker will be used and port 1883 will be exposed. Otherwise, device-monitor will connect to an external MQTT broker running on `DEVICE_IP:1883`.
* If `USE_INTERNAL_AZURITE` is set to `true`, the internal Azurite storage emulator will be used. Otherwise, device-monitor will connect to an external Azurite storage emulator running on `DEVICE_IP:10000`.
* `<TAG>` is the tag of the device-monitor Docker image.


