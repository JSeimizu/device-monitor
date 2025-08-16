# Docker Environment for Device Monitor

## How To build
```bash
cd tools/device-monitor-docker
docker build -f Dockerfile -t device-monitor:<TAG> .
```

## How To Run
```bash
docker run --rm -it \
           -v <WORK_DIR>:/work \
           -e DEVICE_IP=<DEVICE_IP> \
           -p 1883:1883 -p 10000:10000 -p 10001:10001 -p 10002:10002 
           device-monitor:<TAG>
```
where,
* `<WORK_DIR>` is the directory device-monitor is running and the logs will be stored.
* `<DEVICE_IP>` is the local IP address the edge device is supposed to connect to.
* `<TAG>` is the tag of the device-monitor docker image.
