# Device Monitor

`device-monitor` is a TUI tool designed to manage and observe AITRIOS devices
through MQTT communication for debugging purposes. It provides a user-friendly
interface for interacting with devices, logging events, and visualizing device
data in real-time. 


### Key Features:
- **MQTT Communication**: Connect to an MQTT broker to send and receive messages from IoT devices.
- **Terminal UI**: A responsive and interactive terminal-based user interface for monitoring and managing devices.
- **AITRIOS DTDL v2 Support**: Implements the AITRIOS Device Twin Definition Language (DTDl) v2 for device management.
- **EVP v2 Protocol Support**: Supports the EVP v2 protocol and the _Device Control Service_ interface to `SystemApp`.
- **Logging**: Configurable logging with support for different verbosity levels.
- **Customizable**: Easily configurable MQTT broker address and logging options via command-line arguments.
- **Error Handling**: Robust error handling using `error_stack` for detailed error reporting.

### Supposed Usage
This tool is intended to be used in a development environment as follows:

- The AITRIOS device, the device on which MQTT broker is running and the device on
  which this tool is running are connected to the same local network.
  - The MQTT broker is supposed to be running as a daemon service on the device.
- Only 1 AITRIOS device is connected to the development network at a time.
- The MQTT broker is configured to allow connections from local network if
  either the AITRIOS device or this tool is running on a different device.
- AITRIOS device is configured to work in Non-TLS mode and be able to connect to
  the local MQTT broker.

### Command-Line Arguments:
- `--broker` or `-b`: Specifies the MQTT broker address (default: `localhost:1883`).
- `--log` or `-l`: Specifies the log file path for saving logs.
- `--verbose` or `-v`: Increase verbosity level for debugging purposes.

### Example Usage:
```bash
$ device-monitor --broker 192.168.28.3:1883 --log device-monitor.log --verbose
```

*) Suppose MQTT broker is running on the device with IP address of `192.168.28.3`.
