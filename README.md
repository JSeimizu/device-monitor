# Device Monitor

## Table of Contents
- [Introduction](#introduction)
- [Key Features](#key-features)
- [Installation](#installation)
  - [Prerequisites](#prerequisites)
  - [System Setup](#system-setup)
- [Command Parameters](#command-parameters)
  - [Synopsis](#synopsis)
  - [Options](#options)
  - [Verbosity Levels](#verbosity-levels)
  - [Example Usage](#example-usage)
- [User Interface](#user-interface)
- [Device Monitor Docker Image](#device-monitor-docker-image)

## Introduction

Device Monitor is a Terminal User Interface (TUI) tool designed to manage and
observe AITRIOS devices through MQTT communication for debugging purposes. It
provides a user-friendly interface for interacting with devices, logging events,
and visualizing device data in real-time.

## Key Features
- **MQTT Communication**: Connect to an MQTT broker to send and receive messages
  from IoT devices
- **Terminal UI**: A responsive and interactive terminal-based user interface
  for monitoring and managing devices
- **AITRIOS DTDL v2 Support**: Implements the AITRIOS Device Twin Definition
  Language (DTDL) v2 for device management
- **EVP v2 Protocol Support**: Supports the EVP v2 protocol and the Device
  Control Service interface to SystemApp
- **Logging**: Configurable logging with support for different verbosity levels
- **Error Handling**: Robust error handling with detailed error reporting

## Installation

### Prerequisites

Before using Device Monitor, you need to set up the following components:

#### 1. Install Rust
[Install Rust on your system](https://www.rust-lang.org/tools/install) to build the tool.

#### 2. Build Device Monitor
```bash
cargo build --release
```

#### 3. Install MQTT Broker
Install `mosquitto` as the MQTT broker:
```bash
sudo apt install mosquitto
```

Configure mosquitto to accept connections from the local network by creating a
`/etc/mosquitto/conf.d/local_network.conf` file with the following content:
```
listener 1883 0.0.0.0
allow_anonymous true
```

#### 4. Install Azurite
Install and start `Azurite` as the Azure Storage emulator:
```bash
docker run -p 10000:10000 \
           -p 10001:10001 \
           -p 10002:10002 \
           mcr.microsoft.com/azure-storage/azurite
```

### System Setup
This tool is intended to be used in a development environment with:
- An AITRIOS device connected to a local network (only 1 device can be
  monitored at a time)
- The device configured to work in Non-TLS mode and able to connect to an MQTT
  broker on the local network
- An MQTT broker (e.g., Mosquitto) running on the host PC or device
- Azurite local Azure Storage emulator for test storage
- Device Monitor tool

## Command Parameters

### Synopsis
```
Usage: device-monitor [OPTIONS]
```

### Options
- `-b, --broker <BROKER>` - MQTT broker address (default: localhost:1883)
- `-a, --azurite-url <AZURITE_URL>` - Azurite URL (default:
  https://127.0.0.1:10000)
- `-l, --log <LOG>` - Log file path
- `-v, --verbose` - Verbose logging (can be used multiple times for increased
  verbosity)
- `-h, --help` - Print help information
- `-V, --version` - Print version information

### Verbosity Levels
- No `-v`: INFO level logging
- `-v`: DEBUG level logging
- `-vv`: TRACE level logging (maximum verbosity)

### Example Usage

#### Basic Usage
```bash
device-monitor \
    --broker 192.168.28.3:1883 \
    --azurite-url https://192.168.28.4:10000 \
    --log device-monitor.log --verbose
```

#### Maximum Verbosity
```bash
device-monitor \
    -b 192.168.28.3:1883 \
    -a https://192.168.28.4:10000 \
    -l device-monitor.log \
    -vv
```

**Note**: In these examples, the MQTT broker is running on IP address
`192.168.28.3` and Azurite is running on IP address `192.168.28.4`. The port
number for Azurite should match the exported port number of the Azurite
container.

## User Interface

Please refer to the [UI documentation](docs/ui.md) for detailed information
about the user interface.

## Device Monitor Docker Image

Please refer to the [Device Monitor Docker Image](tools/device-monitor-docker/README.md).
