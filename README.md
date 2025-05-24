# Device Monitor

`device-monitor` is a terminal-based monitoring tool designed to manage and
observe AITRIOS devices through MQTT communication for debugging purposes. It
provides a user-friendly interface for interacting with devices, logging events,
and visualizing device data in real-time. The tool is built using Rust and
leverages libraries such as `ratatui` for terminal UI, `crossterm` for input
handling, and `clap` for command-line argument parsing.

### Key Features:
- **MQTT Communication**: Connect to an MQTT broker to send and receive messages from IoT devices.
- **Terminal UI**: A responsive and interactive terminal-based user interface for monitoring and managing devices.
- **EVP v2 Protocol Support**: Supports the EVP v2 protocol and the _Device Control Service_ interface to `SystemApp`.
- **Logging**: Configurable logging with support for different verbosity levels.
- **Customizable**: Easily configurable MQTT broker address and logging options via command-line arguments.
- **Error Handling**: Robust error handling using `error_stack` for detailed error reporting.

### Command-Line Arguments:
- `--broker` or `-b`: Specifies the MQTT broker address (default: `localhost:1883`).
- `--log` or `-l`: Specifies the log file path for saving logs.
- `--verbose` or `-v`: Increase verbosity level for debugging purposes.

### Example Usage:
```bash
$ device-monitor --broker mqtt.example.com:1883 --log device-monitor.log --verbose
