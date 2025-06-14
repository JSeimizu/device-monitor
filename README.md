# Device Monitor

`device-monitor` is a TUI tool designed to manage and observe AITRIOS devices
through MQTT communication for debugging purposes. It provides a user-friendly
interface for interacting with devices, logging events, and visualizing device
data in real-time. 


## Key Features:
- **MQTT Communication**: Connect to an MQTT broker to send and receive messages from IoT devices.
- **Terminal UI**: A responsive and interactive terminal-based user interface for monitoring and managing devices.
- **AITRIOS DTDL v2 Support**: Implements the AITRIOS Device Twin Definition Language (DTDl) v2 for device management.
- **EVP v2 Protocol Support**: Supports the EVP v2 protocol and the _Device Control Service_ interface to `SystemApp`.
- **Logging**: Configurable logging with support for different verbosity levels.
- **Customizable**: Easily configurable MQTT broker address and logging options via command-line arguments.
- **Error Handling**: Robust error handling using `error_stack` for detailed error reporting.

## Supposed Usage
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
- `--azurite-url` or `-a`: Specifies the Azurite URL (default: `https://127.0.1:10000`).
- `--log` or `-l`: Specifies the log file path for saving logs (optional).
- `--verbose` or `-v`: Increase verbosity level for debugging purposes. Can be used multiple times (e.g., `-vv` for maximum verbosity).

### Example Usage:
```bash
# Basic usage
$ device-monitor \
       --broker 192.168.28.3:1883 \
       --azurite-url https://192.168.28.4:10000 \
       --log device-monitor.log --verbose

# Maximum verbosity (trace level)
$ device-monitor \
       -b 192.168.28.3:1883 \
       -a https://192.168.28.4:10000 \
       -l device-monitor.log \
       -vv
```

*) Suppose MQTT broker is running on the device with IP address of
`192.168.28.3` and Azurite is running on the device with IP address of `192.168.28.4`

### Screen Transition

Following is the screen transition of the tool with the key-press:

![Screen Transition](doc/DeviceMonitor.png)

* MainScreen: Main screen.
  * Up/k:       Move focus up.
  * Down/j:     Move focus down.
  * Enter:      Open the focus area in detail screen.
  * d:          Move to direct command screen.
  * m:          Move to EVP module screen.
  * e:          Move to configuration screen of focused area.
  * q:          Move to exit screen.
* DetailScreen: Detail screen of the focused area.
  * d:          Move to direct command screen.
  * m:          Move to EVP module screen.
  * e:          Move to configuration screen of focused area.
  * Esc/Enter:  Move back to previous screen.
  * q:          Move to exit screen.
* DirectCommandScreen: Direct command screen.
  * r:          Move to reboot screen.
  * i:          Move DirectGetImage screen.
  * f:          Move FactoryReset screen.
  * Esc:        Move back to previous screen.
  * q:          Move to exit screen.
* DirectGetImageScreen: Direct get image screen.
  * e:          Edit parameters.
  * w:          Show DirectGetImage command request.
  * s:          Send DirectGetImage command.
  * Esc:        Move back to previous screen.
  * q:          Move to exit screen.
* RebootScreen: Reboot screen.
  * w:          Show Reboot command request.
  * s:          Send Reboot command.
  * Esc:        Move back to previous screen.
  * q:          Move to exit screen.
* FactoryResetScreen: Factory reset screen.
  * w:          Show FactoryReset command request.
  * s:          Send FactoryReset command.
  * Esc:        Move back to previous screen.
  * q:          Move to exit screen.
* EVPModuleScreen: EVP module operating screen.
  * a:          Add a new module.
  * r:          Remove the selected module.
  * d:          Deploy the selected module.
  * Esc:        Move back to previous screen.
  * q:          Move to exit screen.
* ConfigurationScreen: Configuration screen of the focused area.
  * i/a/Enter:  Edit the configuration item.
  * w:          Show the configuration item request.
  * s:          Send the configuration item request.
* ExitScreen: Exit screen.
  * y:          Exit the tool.
  * n:          Move back to previous screen.
