/*
Copyright [2025] Seimizu Joukan

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

pub mod ui;

#[allow(unused)]
use {
    super::{
        app,
        azurite::{
            AzuriteAction, try_reinit_azurite_storage, with_azurite_storage,
            with_azurite_storage_mut,
        },
        error::{DMError, DMErrorExt},
        mqtt_ctrl::evp::module::ModuleInfo,
        mqtt_ctrl::{MqttCtrl, with_mqtt_ctrl, with_mqtt_ctrl_mut},
        ota::FirmwareProperty,
    },
    crate::mqtt_ctrl::evp::edge_app::EdgeAppInfo,
    chrono::Local,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    error_stack::{Report, Result},
    jlogger_tracing::{JloggerBuilder, LevelFilter, LogTimeFormat, jdebug, jerror, jinfo},
    ratatui::{
        DefaultTerminal, Frame, Terminal,
        buffer::Buffer,
        crossterm::{
            event::{DisableMouseCapture, EnableMouseCapture},
            execute,
            terminal::{
                EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
            },
        },
        layout::{Alignment, Rect},
        layout::{Constraint, Layout},
        prelude::{Backend, CrosstermBackend},
        prelude::{Color, Direction, Style},
        style::Stylize,
        symbols::border,
        text::{Line, Span, Text},
        widgets::{Block, Borders, List, ListItem, Paragraph, Widget},
    },
    std::{
        collections::HashMap,
        fmt::Display,
        io,
        sync::{Mutex, OnceLock},
        time::{Duration, Instant},
    },
    ui::*,
};

/// Default timeout for event polling in milliseconds
const DEFAULT_EVENT_POLL_TIMEOUT: u64 = 250;

/// Global App instance protected by mutex for thread safety
static GLOBAL_APP: OnceLock<Mutex<App>> = OnceLock::new();

/// Initialize the global App instance
pub fn init_global_app(cfg: AppConfig) -> Result<(), DMError> {
    let app = App::new(cfg)?;
    GLOBAL_APP.set(Mutex::new(app)).map_err(|_| {
        Report::new(DMError::InvalidData).attach_printable("Global App already initialized")
    })?;
    Ok(())
}

/// Get reference to global App mutex (for internal use)
fn get_global_app_ref() -> &'static Mutex<App> {
    GLOBAL_APP
        .get()
        .expect("Global App not initialized - call init_global_app first")
}

/// Access global App with closure for immutable operations
pub fn with_global_app<F, R>(f: F) -> R
where
    F: FnOnce(&App) -> R,
{
    let app_guard = get_global_app_ref()
        .lock()
        .expect("Failed to lock global App mutex");
    f(&*app_guard)
}

/// Access global App with closure for mutable operations
pub fn with_global_app_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut App) -> R,
{
    let mut app_guard = get_global_app_ref()
        .lock()
        .expect("Failed to lock global App mutex");
    f(&mut *app_guard)
}

/// Application configuration structure containing broker settings
pub struct AppConfig<'a> {
    pub broker: &'a str,
}

/// Different screens/views available in the device monitor application
#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub enum DMScreenState {
    #[default]
    Initial,

    Configuring,
    Completed,
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub enum DMScreen {
    /// Main dashboard view showing device information
    #[default]
    Main,
    /// Module details view
    Module,
    /// Configuration editing screen
    Configuration,
    /// User configuration editing screen
    ConfigurationUser,
    /// Direct command execution screen
    DirectCommand,
    /// EVP module management screen
    EvpModule,
    /// Token provider management screen
    TokenProvider,
    /// Token provider blob viewer
    TokenProviderBlobs,
    /// Event log viewer
    Elog,
    /// Edge application management
    EdgeApp(DMScreenState),
    /// OTA firmware update screen
    Ota,
    /// OTA firmware update configuration screen
    OtaConfig(DMScreenState),
    /// Exit confirmation dialog
    Exiting,
}

#[derive(Debug, Default, PartialEq, PartialOrd, Clone)]
#[repr(usize)]
#[allow(unused)]
pub enum DirectCommand {
    Reboot = 0,
    GetDirectImage,
    FactoryReset,
    ReadSensorRegister,
    WriteSensorRegister,
    ShutDown,

    /// Storage token request command from the device
    StorageTokenRequest(String, String),

    #[default]
    Invalid,
}

impl std::fmt::Display for DirectCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DirectCommand::Reboot => write!(f, "Reboot"),
            DirectCommand::GetDirectImage => write!(f, "GetDirectImage"),
            DirectCommand::FactoryReset => write!(f, "FactoryReset"),
            DirectCommand::ReadSensorRegister => write!(f, "ReadSensorRegister"),
            DirectCommand::WriteSensorRegister => write!(f, "WriteSensorRegister"),
            DirectCommand::ShutDown => write!(f, "ShutDown"),
            DirectCommand::StorageTokenRequest(key, filename) => {
                write!(f, "StorageTokenRequest({}, {})", key, filename)
            }
            DirectCommand::Invalid => write!(f, "InvalidCommand"),
        }
    }
}

#[derive(Debug, Default, PartialEq, PartialOrd, Clone, Copy)]
#[repr(usize)]
#[allow(unused)]
pub enum DirectCommandPara {
    GetDirectImageSensorName = 0,
    GetDirectImageNetworkId,

    #[default]
    Invalid,
}

/// Configuration keys for the device
/// These keys are used to identify the configuration parameters
/// and are used to parse the configuration file
///
/// IMPORTANT: Don't change the order of the keys!
#[derive(Debug, Default, PartialEq, PartialOrd, Clone, Copy)]
#[repr(usize)]
pub enum ConfigKey {
    //AgentState
    ReportStatusIntervalMin = 0,
    ReportStatusIntervalMax,

    //SystemSettings
    LedEnabled,
    TemperatureUpdateInterval,
    AllLogSettingLevel,
    AllLogSettingDestination,
    AllLogSettingStorageName,
    AllLogSettingPath,
    MainLogSettingLevel,
    MainLogSettingDestination,
    MainLogSettingStorageName,
    MainLogSettingPath,
    SensorLogSettingLevel,
    SensorLogSettingDestination,
    SensorLogSettingStorageName,
    SensorLogSettingPath,
    CompanionFwLogSettingLevel,
    CompanionFwLogSettingDestination,
    CompanionFwLogSettingStorageName,
    CompanionFwLogSettingPath,
    CompanionAppLogSettingLevel,
    CompanionAppLogSettingDestination,
    CompanionAppLogSettingStorageName,
    CompanionAppLogSettingPath,

    //Network settings
    IpMethod,
    NtpUrl,
    StaticIpv4Ip,
    StaticIpv4SubnetMask,
    StaticIpv4Gateway,
    StaticIpv4Dns,
    StaticIpv6Ip,
    StaticIpv6SubnetMask,
    StaticIpv6Gateway,
    StaticIpv6Dns,
    ProxyUrl,
    ProxyPort,
    ProxyUserName,
    ProxyPassword,

    // Wireless settings
    StaSsid,
    StaPassword,
    StaEncryption,

    // DirectCommandPara
    DirectGetImageSensorName,
    DirectGetImageNetworkId,

    // Edge App
    CommonSettingsProcessState,
    CommonSettingsLogLevel,
    CommonSettingsISNumberOfIterations,
    CommonSettingsPQCameraImageSizeWidth,
    CommonSettingsPQCameraImageSizeHeight,
    CommonSettingsPQCameraImageSizeScalingPolicy,
    CommonSettingsPQFrameRateNum,
    CommonSettingsPQFrameRateDenom,
    CommonSettingsPQDigitalZoom,
    CommonSettingsPQCameraImageFlipHorizontal,
    CommonSettingsPQCameraImageFlipVertical,
    CommonSettingsPQExposureMode,
    // Auto exposure
    CommonSettingsPQAeMaxExposureTime,
    CommonSettingsPQAeMinExposureTime,
    CommonSettingsPQAeMaxGain,
    CommonSettingsPQAeConvergenceSpeed,
    CommonSettingsPQEvCompensation,
    // Auto exposure anti-flicker
    CommonSettingsPQAeAntiFlickerMode,
    // Manual exposure
    CommonSettingsPQMeExposureTime,
    CommonSettingsPQMeGain,
    CommonSettingsPQWhiteBalanceMode,
    // Auto white balance
    CommonSettingsPQAwbConvergenceSpeed,
    // Manual white balance preset
    CommonSettingsPQMWBPColorTemperature,
    // Manual white balance gain
    CommonSettingsPQMWBGRed,
    CommonSettingsPQMWBGBlue,
    // Image cropping
    CommonSettingsPQICLeft,
    CommonSettingsPQICTop,
    CommonSettingsPQICWidth,
    CommonSettingsPQICHeight,
    // Image rotation
    CommonSettingsPQImageRotation,

    // Port settings
    CommonSettingsPSMetadataMethod,
    CommonSettingsPSMetadataStorageName,
    CommonSettingsPSMetadataEndpoint,
    CommonSettingsPSMetadataPath,
    CommonSettingsPSMetadataEnabled,

    // Port settings for input tensor
    CommonSettingsPSITMethod,
    CommonSettingsPSITStorageName,
    CommonSettingsPSITEndpoint,
    CommonSettingsPSITPath,
    CommonSettingsPSITEnabled,

    // Codec settings
    CommonSettingsCSFormat,

    CommonSettingsNumberOfInferencePerMessage,
    CommonSettingsUploadInterval,

    // OTA
    OtaMainChipLoaderChip,
    OtaMainChipLoaderVersion,
    OtaMainChipLoaderPackageUrl,
    OtaMainChipLoaderHash,
    OtaMainChipLoaderSize,
    OtaMainChipFirmwareChip,
    OtaMainChipFirmwareVersion,
    OtaMainChipFirmwarePackageUrl,
    OtaMainChipFirmwareHash,
    OtaMainChipFirmwareSize,

    OtaCompanionChipLoaderChip,
    OtaCompanionChipLoaderVersion,
    OtaCompanionChipLoaderPackageUrl,
    OtaCompanionChipLoaderHash,
    OtaCompanionChipLoaderSize,
    OtaCompanionChipFirmwareChip,
    OtaCompanionChipFirmwareVersion,
    OtaCompanionChipFirmwarePackageUrl,
    OtaCompanionChipFirmwareHash,
    OtaCompanionChipFirmwareSize,

    OtaSensorChipLoaderChip,
    OtaSensorChipLoaderVersion,
    OtaSensorChipLoaderPackageUrl,
    OtaSensorChipLoaderHash,
    OtaSensorChipLoaderSize,
    OtaSensorChipFirmwareChip,
    OtaSensorChipFirmwareVersion,
    OtaSensorChipFirmwarePackageUrl,
    OtaSensorChipFirmwareHash,
    OtaSensorChipFirmwareSize,

    #[default]
    Invalid,
}

impl From<ConfigKey> for usize {
    fn from(value: ConfigKey) -> Self {
        value as usize
    }
}

impl From<usize> for ConfigKey {
    fn from(value: usize) -> Self {
        if value >= ConfigKey::size() {
            return ConfigKey::Invalid;
        }

        // SAFETY: We've verified that value is within the valid range for ConfigKey variants
        unsafe { std::mem::transmute(value) }
    }
}

impl Display for ConfigKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            ConfigKey::ReportStatusIntervalMin => "report-status-interval-min",
            ConfigKey::ReportStatusIntervalMax => "report-status-interval-max",
            ConfigKey::LedEnabled => "led_enabled",
            ConfigKey::TemperatureUpdateInterval => "temperature_update_interval",
            ConfigKey::AllLogSettingLevel => "log.all.level",
            ConfigKey::AllLogSettingDestination => "log.all.destination",
            ConfigKey::AllLogSettingStorageName => "log.all.storage_name",
            ConfigKey::AllLogSettingPath => "log.all.path",
            ConfigKey::MainLogSettingLevel => "log.main.level",
            ConfigKey::MainLogSettingDestination => "log.main.destination",
            ConfigKey::MainLogSettingStorageName => "log.main.storage_name",
            ConfigKey::MainLogSettingPath => "log.main.path",
            ConfigKey::SensorLogSettingLevel => "log.sensor.level",
            ConfigKey::SensorLogSettingDestination => "log.sensor.destination",
            ConfigKey::SensorLogSettingStorageName => "log.sensor.storage_name",
            ConfigKey::SensorLogSettingPath => "log.sensor.path",
            ConfigKey::CompanionFwLogSettingLevel => "log.companion_fw.level",
            ConfigKey::CompanionFwLogSettingDestination => "log.companion_fw.destination",
            ConfigKey::CompanionFwLogSettingStorageName => "log.companion_fw.storage_name",
            ConfigKey::CompanionFwLogSettingPath => "log.companion_fw.path",
            ConfigKey::CompanionAppLogSettingLevel => "log.companion_app.level",
            ConfigKey::CompanionAppLogSettingDestination => "log.companion_app.destination",
            ConfigKey::CompanionAppLogSettingStorageName => "log.companion_app.storage_name",
            ConfigKey::CompanionAppLogSettingPath => "log.companion_app.path",

            ConfigKey::IpMethod => "ip_method",
            ConfigKey::NtpUrl => "ntp_url",
            ConfigKey::StaticIpv4Ip => "static_ipv4_ip",
            ConfigKey::StaticIpv4SubnetMask => "static_ipv4_subnet_mask",
            ConfigKey::StaticIpv4Gateway => "static_ipv4_gateway",
            ConfigKey::StaticIpv4Dns => "static_ipv4_dns",
            ConfigKey::StaticIpv6Ip => "static_ipv6_ip",
            ConfigKey::StaticIpv6SubnetMask => "static_ipv6_subnet_mask",
            ConfigKey::StaticIpv6Gateway => "static_ipv6_gateway",
            ConfigKey::StaticIpv6Dns => "static_ipv6_dns",
            ConfigKey::ProxyUrl => "proxy_url",
            ConfigKey::ProxyPort => "proxy_port",
            ConfigKey::ProxyUserName => "proxy_user_name",
            ConfigKey::ProxyPassword => "proxy_password",
            ConfigKey::StaSsid => "station_mode_ssid",
            ConfigKey::StaPassword => "station_mode_password",
            ConfigKey::StaEncryption => "station_mode_encryption",
            ConfigKey::DirectGetImageSensorName => "sensor_name",
            ConfigKey::DirectGetImageNetworkId => "network_id",

            ConfigKey::CommonSettingsProcessState => "process_state",
            ConfigKey::CommonSettingsLogLevel => "log_level",
            ConfigKey::CommonSettingsISNumberOfIterations => "number_of_iterations",
            ConfigKey::CommonSettingsPQCameraImageSizeWidth => "PQ.camera_image_size.width",
            ConfigKey::CommonSettingsPQCameraImageSizeHeight => "PQ.camera_image_size.height",
            ConfigKey::CommonSettingsPQCameraImageSizeScalingPolicy => {
                "PQ.camera_image_size.scaling_policy"
            }
            ConfigKey::CommonSettingsPQFrameRateNum => "PQ.frame_rate.num",
            ConfigKey::CommonSettingsPQFrameRateDenom => "PQ.frame_rate.denom",
            ConfigKey::CommonSettingsPQDigitalZoom => "PQ.digital_zoom",
            ConfigKey::CommonSettingsPQCameraImageFlipHorizontal => {
                "PQ.camera_image_flip_horizontal"
            }
            ConfigKey::CommonSettingsPQCameraImageFlipVertical => "PQ.camera_image_flip_vertical",
            ConfigKey::CommonSettingsPQExposureMode => "PQ.exposure_mode",
            ConfigKey::CommonSettingsPQAeMaxExposureTime => "PQ.auto_exposure.max_exposure_time",
            ConfigKey::CommonSettingsPQAeMinExposureTime => "PQ.auto_exposure.min_exposure_time",
            ConfigKey::CommonSettingsPQAeMaxGain => "PQ.auto_exposure.max_gain",
            ConfigKey::CommonSettingsPQAeConvergenceSpeed => "PQ.auto_exposure.convergence_speed",
            ConfigKey::CommonSettingsPQEvCompensation => "PQ.ev_compensation",

            ConfigKey::CommonSettingsPQAeAntiFlickerMode => "PQ.ae_anti_flicker_mode",
            ConfigKey::CommonSettingsPQMeExposureTime => "PQ.manual_exposure.exposure_time",
            ConfigKey::CommonSettingsPQMeGain => "PQ.manual_exposure.gain",
            ConfigKey::CommonSettingsPQWhiteBalanceMode => "PQ.white_balance_mode",
            ConfigKey::CommonSettingsPQAwbConvergenceSpeed => "PQ.auto_wb.convergence_speed",
            ConfigKey::CommonSettingsPQMWBPColorTemperature => "PQ.manual_wb.color_temperature",
            ConfigKey::CommonSettingsPQMWBGRed => "PQ.manual_wb.gain_red",
            ConfigKey::CommonSettingsPQMWBGBlue => "PQ.manual_wb.gain_blue",
            ConfigKey::CommonSettingsPQICLeft => "PQ.image_cropping.left",
            ConfigKey::CommonSettingsPQICTop => "PQ.image_cropping.top",
            ConfigKey::CommonSettingsPQICWidth => "PQ.image_cropping.width",
            ConfigKey::CommonSettingsPQICHeight => "PQ.image_cropping.height",
            ConfigKey::CommonSettingsPQImageRotation => "PQ.image_rotation",
            ConfigKey::CommonSettingsPSMetadataMethod => "port_settings.OT.method",
            ConfigKey::CommonSettingsPSMetadataStorageName => "port_settings.OT.storage_name",
            ConfigKey::CommonSettingsPSMetadataEndpoint => "port_settings.OT.endpoint",
            ConfigKey::CommonSettingsPSMetadataPath => "port_settings.OT.path",
            ConfigKey::CommonSettingsPSMetadataEnabled => "port_settings.OT.enabled",
            ConfigKey::CommonSettingsPSITMethod => "port_settings.IT.method",
            ConfigKey::CommonSettingsPSITStorageName => "port_settings.IT.storage_name",
            ConfigKey::CommonSettingsPSITEndpoint => "port_settings.IT.endpoint",
            ConfigKey::CommonSettingsPSITPath => "port_settings.IT.path",
            ConfigKey::CommonSettingsPSITEnabled => "port_settings.IT.enabled",
            ConfigKey::CommonSettingsCSFormat => "codec_settings.format",
            ConfigKey::CommonSettingsNumberOfInferencePerMessage => {
                "number_of_inference_per_message"
            }
            ConfigKey::CommonSettingsUploadInterval => "upload_interval",

            ConfigKey::OtaMainChipLoaderChip => "main_chip.loader.chip",
            ConfigKey::OtaMainChipLoaderVersion => "main_chip.loader.version",
            ConfigKey::OtaMainChipLoaderPackageUrl => "main_chip.loader.package_url",
            ConfigKey::OtaMainChipLoaderHash => "main_chip.loader.hash",
            ConfigKey::OtaMainChipLoaderSize => "main_chip.loader.size",
            ConfigKey::OtaMainChipFirmwareChip => "main_chip.firmware.chip",
            ConfigKey::OtaMainChipFirmwareVersion => "main_chip.firmware.version",
            ConfigKey::OtaMainChipFirmwarePackageUrl => "main_chip.firmware.package_url",
            ConfigKey::OtaMainChipFirmwareHash => "main_chip.firmware.hash",
            ConfigKey::OtaMainChipFirmwareSize => "main_chip.firmware.size",

            ConfigKey::OtaCompanionChipLoaderChip => "companion_chip.loader.chip",
            ConfigKey::OtaCompanionChipLoaderVersion => "companion_chip.loader.version",
            ConfigKey::OtaCompanionChipLoaderPackageUrl => "companion_chip.loader.package_url",
            ConfigKey::OtaCompanionChipLoaderHash => "companion_chip.loader.hash",
            ConfigKey::OtaCompanionChipLoaderSize => "companion_chip.loader.size",
            ConfigKey::OtaCompanionChipFirmwareChip => "companion_chip.firmware.chip",
            ConfigKey::OtaCompanionChipFirmwareVersion => "companion_chip.firmware.version",
            ConfigKey::OtaCompanionChipFirmwarePackageUrl => "companion_chip.firmware.package_url",
            ConfigKey::OtaCompanionChipFirmwareHash => "companion_chip.firmware.hash",
            ConfigKey::OtaCompanionChipFirmwareSize => "companion_chip.firmware.size",

            ConfigKey::OtaSensorChipLoaderChip => "sensor_chip.loader.chip",
            ConfigKey::OtaSensorChipLoaderVersion => "sensor_chip.loader.version",
            ConfigKey::OtaSensorChipLoaderPackageUrl => "sensor_chip.loader.package_url",
            ConfigKey::OtaSensorChipLoaderHash => "sensor_chip.loader.hash",
            ConfigKey::OtaSensorChipLoaderSize => "sensor_chip.loader.size",
            ConfigKey::OtaSensorChipFirmwareChip => "sensor_chip.firmware.chip",
            ConfigKey::OtaSensorChipFirmwareVersion => "sensor_chip.firmware.version",
            ConfigKey::OtaSensorChipFirmwarePackageUrl => "sensor_chip.firmware.package_url",
            ConfigKey::OtaSensorChipFirmwareHash => "sensor_chip.firmware.hash",
            ConfigKey::OtaSensorChipFirmwareSize => "sensor_chip.firmware.size",

            _ => "Invalid",
        };

        write!(f, "{}", msg)
    }
}

impl ConfigKey {
    // Returns the number of configuration keys including the invalid key
    // Note ConfigKey is used as index in the config_keys vector starting from 0
    pub fn size() -> usize {
        ConfigKey::Invalid as usize + 1
    }

    pub fn note(&self) -> &'static str {
        match self {
            ConfigKey::AllLogSettingLevel => {
                "0: critical, 1: error, 2: warning, 3: info, 4: debug, 5: trace"
            }
            ConfigKey::AllLogSettingDestination => "0: uart, 1: cloud_storage",
            ConfigKey::AllLogSettingStorageName => "EVP Token provider ID.",
            ConfigKey::MainLogSettingLevel => {
                "0: critical, 1: error, 2: warning, 3: info, 4: debug, 5: trace"
            }
            ConfigKey::MainLogSettingDestination => "0: uart, 1: cloud_storage",
            ConfigKey::MainLogSettingStorageName => "EVP Token provider ID.",
            ConfigKey::SensorLogSettingLevel => {
                "0: critical, 1: error, 2: warning, 3: info, 4: debug, 5: trace"
            }
            ConfigKey::SensorLogSettingDestination => "0: uart, 1: cloud_storage",
            ConfigKey::SensorLogSettingStorageName => "EVP Token provider ID.",
            ConfigKey::CompanionFwLogSettingLevel => "Log level for companion firmware log",
            ConfigKey::CompanionFwLogSettingDestination => "0: uart, 1: cloud_storage",

            ConfigKey::CompanionFwLogSettingStorageName => "EVP Token provider ID.",
            ConfigKey::CompanionAppLogSettingLevel => {
                "0: critical, 1: error, 2: warning, 3: info, 4: debug, 5: trace"
            }
            ConfigKey::CompanionAppLogSettingDestination => "0: uart, 1: cloud_storage",
            ConfigKey::CompanionAppLogSettingStorageName => "EVP Token provider ID.",

            // Network settings
            ConfigKey::IpMethod => "0: dhcp, 1: static",
            ConfigKey::NtpUrl => "Domain name or IP address",
            ConfigKey::ProxyUrl => "Domain name or IP address",
            ConfigKey::StaEncryption => "0: wpa2_psk, 1: wpa3_psk, 2: wpa2_wpa3_psk'",

            // Edge App
            ConfigKey::CommonSettingsProcessState => "0: stopped, 1: running",
            ConfigKey::CommonSettingsLogLevel => {
                "0: critical, 1: error, 2: warning, 3: info, 4: debug, 5: trace"
            }
            ConfigKey::CommonSettingsPQCameraImageSizeScalingPolicy => {
                "1: sensitivity, 2: resolution"
            }
            ConfigKey::CommonSettingsPQCameraImageFlipHorizontal => "0: normal, 1: flip",
            ConfigKey::CommonSettingsPQCameraImageFlipVertical => "0: normal, 1: flip",
            ConfigKey::CommonSettingsPQExposureMode => "0: auto, 1: manual",
            ConfigKey::CommonSettingsPQAeAntiFlickerMode => "0: off, 1: auto, 2: 50Hz, 3: 60Hz",
            ConfigKey::CommonSettingsPQWhiteBalanceMode => "0: auto, 1: preset",
            ConfigKey::CommonSettingsPQAwbConvergenceSpeed => "4300K ~ 5600K",
            ConfigKey::CommonSettingsPQMWBPColorTemperature => {
                "0: 3200K, 1: 4300K, 2: 5600K, 3: 6500K"
            }

            ConfigKey::CommonSettingsPQMWBGRed | ConfigKey::CommonSettingsPQMWBGBlue => {
                "manual white balance"
            }

            ConfigKey::CommonSettingsPSMetadataEndpoint
            | ConfigKey::CommonSettingsPSMetadataPath
            | ConfigKey::CommonSettingsPSMetadataEnabled => "output tensor/ metadata",

            ConfigKey::CommonSettingsPSITEndpoint
            | ConfigKey::CommonSettingsPSITPath
            | ConfigKey::CommonSettingsPSITEnabled => " input tensor / raw data",

            ConfigKey::CommonSettingsPQImageRotation => {
                "0: none, 1: clockwise 90 degrees, 2: clockwise 180 degrees, 3: clockwise 270 degrees"
            }
            ConfigKey::CommonSettingsPSMetadataMethod => {
                "0: evp telemetry, 1: blob storage, 2: http storage"
            }
            ConfigKey::CommonSettingsPSMetadataStorageName => "EVP Token provider ID.",
            ConfigKey::CommonSettingsPSITMethod => {
                "0: evp telemetry, 1: blob storage, 2: http storage"
            }
            ConfigKey::CommonSettingsPSITStorageName => "EVP Token provider ID.",
            ConfigKey::CommonSettingsCSFormat => "1: jpeg",

            ConfigKey::OtaMainChipLoaderChip | ConfigKey::OtaMainChipFirmwareChip => {
                "default: ApFw"
            }
            ConfigKey::OtaCompanionChipLoaderChip | ConfigKey::OtaCompanionChipFirmwareChip => {
                "default: AI-ISP"
            }
            ConfigKey::OtaSensorChipLoaderChip | ConfigKey::OtaSensorChipFirmwareChip => {
                "default: IMX500"
            }
            _ => "",
        }
    }

    pub fn is_sas_url_entry(&self) -> bool {
        match self {
            ConfigKey::OtaMainChipLoaderPackageUrl
            | ConfigKey::OtaMainChipFirmwarePackageUrl
            | ConfigKey::OtaCompanionChipLoaderPackageUrl
            | ConfigKey::OtaCompanionChipFirmwarePackageUrl
            | ConfigKey::OtaSensorChipLoaderPackageUrl
            | ConfigKey::OtaSensorChipFirmwarePackageUrl => true,
            _ => false,
        }
    }
}

/// Focus areas within the main window for navigation
#[derive(Debug, Default, PartialEq, PartialOrd, Clone, Copy)]
pub enum MainWindowFocus {
    #[default]
    MainChip,
    CompanionChip,
    SensorChip,
    DeviceManifest,
    AgentState,
    DeploymentStatus,
    DeviceReserved,
    DeviceState,
    DeviceCapabilities,
    SystemSettings,
    NetworkSettings,
    WirelessSettings,
}

impl MainWindowFocus {
    /// Navigation order for main window focus areas
    const NAVIGATION_ORDER: [MainWindowFocus; 12] = [
        MainWindowFocus::MainChip,
        MainWindowFocus::CompanionChip,
        MainWindowFocus::SensorChip,
        MainWindowFocus::DeviceManifest,
        MainWindowFocus::AgentState,
        MainWindowFocus::DeploymentStatus,
        MainWindowFocus::DeviceReserved,
        MainWindowFocus::DeviceState,
        MainWindowFocus::DeviceCapabilities,
        MainWindowFocus::SystemSettings,
        MainWindowFocus::NetworkSettings,
        MainWindowFocus::WirelessSettings,
    ];

    /// Get the next focus in navigation order
    pub fn next(&self) -> Self {
        let current_index = Self::NAVIGATION_ORDER
            .iter()
            .position(|&focus| focus == *self)
            .unwrap_or(0);
        let next_index = (current_index + 1) % Self::NAVIGATION_ORDER.len();
        Self::NAVIGATION_ORDER[next_index]
    }

    /// Get the previous focus in navigation order
    pub fn previous(&self) -> Self {
        let current_index = Self::NAVIGATION_ORDER
            .iter()
            .position(|&focus| focus == *self)
            .unwrap_or(0);
        let prev_index = if current_index == 0 {
            Self::NAVIGATION_ORDER.len() - 1
        } else {
            current_index - 1
        };
        Self::NAVIGATION_ORDER[prev_index]
    }
    pub fn user_config_file(&self) -> &'static str {
        match self {
            MainWindowFocus::DeploymentStatus => "edge_app_deploy.json",
            MainWindowFocus::SystemSettings => "system_settings.json",
            MainWindowFocus::NetworkSettings => "network_settings.json",
            MainWindowFocus::WirelessSettings => "wireless_settings.json",
            MainWindowFocus::MainChip
            | MainWindowFocus::SensorChip
            | MainWindowFocus::CompanionChip => "{ota_fw, ota_ai_model}.json",
            _ => "configure.json",
        }
    }
}

/// Main application state and controller
pub struct App {
    exit: bool,
    screens: Vec<DMScreen>,
    main_window_focus: MainWindowFocus,
    config_keys: Vec<String>,
    config_key_focus: usize,
    config_key_focus_start: usize,
    config_key_focus_end: usize,
    /// Since companion chip and sensor chip shares the same display region in main ui,
    /// tracks the last focused chip because the companion chip and sensor chip share the same
    /// display region.
    last_config_companion_sensor: usize,
    config_key_editable: bool,
    config_result: Option<Result<String, DMError>>,
    app_error: Option<String>,
    token_provider_for_config: Option<ConfigKey>,
    blob_list_state: Option<ui::ui_token_provider_blobs::BlobListState>,
}

impl App {
    fn is_log_storage_config_key(config_key: ConfigKey) -> bool {
        matches!(
            config_key,
            ConfigKey::AllLogSettingStorageName
                | ConfigKey::MainLogSettingStorageName
                | ConfigKey::SensorLogSettingStorageName
                | ConfigKey::CompanionFwLogSettingStorageName
        )
    }
    /// Creates a new application instance with the given configuration
    pub fn new(cfg: AppConfig) -> Result<Self, DMError> {
        Ok(Self {
            exit: false,
            screens: vec![DMScreen::Main],
            main_window_focus: MainWindowFocus::default(),
            // Initialize config keys with empty strings excluding the invalid key
            config_keys: (0..ConfigKey::size()).map(|_| String::new()).collect(),
            config_key_focus: 0,
            config_key_focus_start: 0,
            config_key_focus_end: 0,
            config_key_editable: false,
            last_config_companion_sensor: MainWindowFocus::CompanionChip as usize,
            config_result: None,
            app_error: None,
            token_provider_for_config: None,
            blob_list_state: None,
        })
    }

    /// Returns the configuration directory path, checking environment variables in order:
    /// DM_CONFIG_DIR, HOME, PWD
    pub fn config_dir() -> String {
        if let Ok(config_dir) = std::env::var("DM_CONFIG_DIR") {
            config_dir
        } else if let Ok(config_dir) = std::env::var("HOME") {
            config_dir
        } else {
            std::env::var("PWD").unwrap().to_owned()
        }
    }

    pub fn current_screen(&self) -> DMScreen {
        self.screens.last().unwrap().to_owned()
    }

    pub fn dm_screen_update(&mut self, screen: DMScreen) {
        if self.screens.len() > 1 {
            self.screens.pop();
        }
        self.screens.push(screen);
    }

    pub fn dm_screen_move_to(&mut self, next_screen: DMScreen) {
        self.screens.push(next_screen);
        self.app_error = None;
        with_mqtt_ctrl_mut(|mqtt_ctrl| mqtt_ctrl.info = None);
    }

    fn update_ota_config_for_url(
        &mut self,
        url_key: ConfigKey,
        hash_key: ConfigKey,
        size_key: ConfigKey,
        module: &ModuleInfo,
    ) {
        self.config_keys[url_key as usize] = module.sas_url.clone();
        self.config_keys[hash_key as usize] = module.hash.clone();
        self.config_keys[size_key as usize] = module.size.to_string();
    }

    pub fn update_sas_url_entries(&mut self) {
        let config_key = ConfigKey::from(self.config_key_focus);
        if !config_key.is_sas_url_entry() {
            return;
        }

        with_azurite_storage(|az| {
            if let Some(module) = az.current_module() {
                match config_key {
                    ConfigKey::OtaMainChipLoaderPackageUrl => {
                        self.update_ota_config_for_url(
                            ConfigKey::OtaMainChipLoaderPackageUrl,
                            ConfigKey::OtaMainChipLoaderHash,
                            ConfigKey::OtaMainChipLoaderSize,
                            module,
                        );
                    }
                    ConfigKey::OtaMainChipFirmwarePackageUrl => {
                        self.update_ota_config_for_url(
                            ConfigKey::OtaMainChipFirmwarePackageUrl,
                            ConfigKey::OtaMainChipFirmwareHash,
                            ConfigKey::OtaMainChipFirmwareSize,
                            module,
                        );
                    }
                    ConfigKey::OtaCompanionChipLoaderPackageUrl => {
                        self.update_ota_config_for_url(
                            ConfigKey::OtaCompanionChipLoaderPackageUrl,
                            ConfigKey::OtaCompanionChipLoaderHash,
                            ConfigKey::OtaCompanionChipLoaderSize,
                            module,
                        );
                    }
                    ConfigKey::OtaCompanionChipFirmwarePackageUrl => {
                        self.update_ota_config_for_url(
                            ConfigKey::OtaCompanionChipFirmwarePackageUrl,
                            ConfigKey::OtaCompanionChipFirmwareHash,
                            ConfigKey::OtaCompanionChipFirmwareSize,
                            module,
                        );
                    }
                    ConfigKey::OtaSensorChipLoaderPackageUrl => {
                        self.update_ota_config_for_url(
                            ConfigKey::OtaSensorChipLoaderPackageUrl,
                            ConfigKey::OtaSensorChipLoaderHash,
                            ConfigKey::OtaSensorChipLoaderSize,
                            module,
                        );
                    }
                    ConfigKey::OtaSensorChipFirmwarePackageUrl => {
                        self.update_ota_config_for_url(
                            ConfigKey::OtaSensorChipFirmwarePackageUrl,
                            ConfigKey::OtaSensorChipFirmwareHash,
                            ConfigKey::OtaSensorChipFirmwareSize,
                            module,
                        );
                    }
                    _ => {}
                }
            }
        });
    }

    pub fn dm_screen_move_back(&mut self) {
        if self.screens.len() > 1 {
            self.screens.pop();
        }

        self.app_error = None;
        with_mqtt_ctrl_mut(|mqtt_ctrl| mqtt_ctrl.info = None);

        // Clear the config keys and ModuleInfo when moving back to Main
        match self.current_screen() {
            DMScreen::Main | DMScreen::Module => {
                self.config_key_clear();
                with_mqtt_ctrl_mut(|mqtt_ctrl| mqtt_ctrl.direct_command_clear());
                with_azurite_storage_mut(|azurite_storage| {
                    azurite_storage.current_module_focus_init();
                    azurite_storage.pop_action();
                });
            }
            _ => {}
        }
    }

    pub fn config_focus_up(&mut self) {
        jdebug!(
            func = "config_focus_up",
            start = self.config_key_focus_start,
            end = self.config_key_focus_end,
            current = self.config_key_focus
        );
        if self.config_key_focus == self.config_key_focus_start {
            self.config_key_focus = self.config_key_focus_end;
        } else {
            self.config_key_focus -= 1;
        }
    }

    pub fn config_focus_down(&mut self) {
        jdebug!(
            func = "config_focus_down",
            start = self.config_key_focus_start,
            end = self.config_key_focus_end,
            current = self.config_key_focus
        );
        if self.config_key_focus == self.config_key_focus_end {
            self.config_key_focus = self.config_key_focus_start;
        } else {
            self.config_key_focus += 1;
        }
    }

    /// Clears all configuration input fields and resets the config result
    pub fn config_key_clear(&mut self) {
        self.config_keys = (0..ConfigKey::size()).map(|_| String::new()).collect();
        self.config_result = None;
    }

    pub fn switch_to_evp_module_screen(&mut self, action: AzuriteAction) {
        // Retrieve module information from Azurite storage when moving to EvpModule screen
        if let Some(result) =
            with_azurite_storage_mut(|azurite_storage| azurite_storage.update_modules(None))
        {
            if let Err(e) = result {
                self.app_error = Some(format!(
                    "Failed to update modules from Azurite: {}",
                    e.error_str().unwrap_or("Unknown error".to_owned())
                ));
            } else {
                with_azurite_storage_mut(|azurite_storage| {
                    azurite_storage.current_module_focus_init();
                    azurite_storage.push_action(action);
                });
                self.dm_screen_move_to(DMScreen::EvpModule);
            }
        }
    }

    pub fn switch_to_token_provider_screen(&mut self) {
        // Scan token providers from Azurite storage when moving to TokenProvider screen
        if let Some(result) =
            with_azurite_storage_mut(|azurite_storage| azurite_storage.scan_upload_containers())
        {
            if let Err(e) = result {
                self.app_error = Some(format!(
                    "Failed to scan token providers from Azurite: {}",
                    e.error_str().unwrap_or("Unknown error".to_owned())
                ));
            } else {
                with_azurite_storage_mut(|azurite_storage| {
                    azurite_storage.current_token_provider_focus_init();
                });
                self.dm_screen_move_to(DMScreen::TokenProvider);
            }
        }
    }

    pub fn switch_to_edge_app_screen(&mut self) {
        let has_instances = with_mqtt_ctrl(|mqtt_ctrl| {
            if let Some(status) = mqtt_ctrl.deployment_status() {
                !status.instances().is_empty()
            } else {
                false
            }
        });
        if has_instances {
            self.dm_screen_move_to(DMScreen::EdgeApp(DMScreenState::Initial))
        } else {
            self.app_error = Some("No Edge App instances found.".to_owned());
        }
    }

    pub fn switch_to_elog_screen(&mut self) {
        if with_mqtt_ctrl(|mqtt_ctrl| mqtt_ctrl.is_device_connected()) {
            self.dm_screen_move_to(DMScreen::Elog);
        } else {
            self.app_error = Some("Device is not connected.".to_owned());
        }
    }

    pub fn switch_to_direct_command_screen(&mut self) {
        if with_mqtt_ctrl(|mqtt_ctrl| mqtt_ctrl.is_device_connected()) {
            self.config_key_clear();
            with_mqtt_ctrl_mut(|mqtt_ctrl| mqtt_ctrl.direct_command_clear());
            self.dm_screen_move_to(DMScreen::DirectCommand);
        } else {
            self.app_error = Some("Device is not connected.".to_owned());
        }
    }

    pub fn switch_to_config_screen(&mut self, user_config: bool) {
        if with_mqtt_ctrl(|mqtt_ctrl| mqtt_ctrl.is_device_connected()) {
            self.config_key_clear();
            if user_config {
                match self.main_window_focus {
                    MainWindowFocus::MainChip
                    | MainWindowFocus::SensorChip
                    | MainWindowFocus::CompanionChip
                    | MainWindowFocus::DeploymentStatus
                    | MainWindowFocus::SystemSettings
                    | MainWindowFocus::NetworkSettings
                    | MainWindowFocus::WirelessSettings => {
                        self.dm_screen_move_to(DMScreen::ConfigurationUser);
                    }
                    _ => {}
                }
            } else {
                match self.main_window_focus {
                    MainWindowFocus::AgentState => {
                        self.config_key_focus_start = ConfigKey::ReportStatusIntervalMin.into();
                        self.config_key_focus_end = ConfigKey::ReportStatusIntervalMax.into();
                        self.config_key_focus = self.config_key_focus_start;
                        self.dm_screen_move_to(DMScreen::Configuration);
                    }
                    MainWindowFocus::SystemSettings => {
                        self.config_key_focus_start = ConfigKey::LedEnabled.into();
                        self.config_key_focus_end = ConfigKey::CompanionAppLogSettingPath.into();
                        self.config_key_focus = self.config_key_focus_start;
                        self.dm_screen_move_to(DMScreen::Configuration);
                    }
                    MainWindowFocus::NetworkSettings => {
                        self.config_key_focus_start = ConfigKey::IpMethod.into();
                        self.config_key_focus_end = ConfigKey::ProxyPassword.into();
                        self.config_key_focus = self.config_key_focus_start;
                        self.dm_screen_move_to(DMScreen::Configuration);
                    }
                    MainWindowFocus::WirelessSettings => {
                        self.config_key_focus_start = ConfigKey::StaSsid.into();
                        self.config_key_focus_end = ConfigKey::StaEncryption.into();
                        self.config_key_focus = self.config_key_focus_start;
                        self.dm_screen_move_to(DMScreen::Configuration);
                    }
                    _ => {}
                }
            }
        } else {
            self.app_error = Some("Device is not connected.".to_owned());
        }
    }

    fn switch_to_ota_config_screen(&mut self, state: DMScreenState) {
        if state == DMScreenState::Initial {
            self.config_key_clear();
            self.config_key_focus_start = ConfigKey::OtaMainChipLoaderChip.into();
            self.config_key_focus_end = ConfigKey::OtaSensorChipFirmwareSize.into();
            self.config_key_focus = self.config_key_focus_start;
        }
        self.dm_screen_move_to(DMScreen::OtaConfig(state));
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) {
        match self.current_screen() {
            DMScreen::Main => {
                match key_event.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.main_window_focus = self.main_window_focus.previous();
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.main_window_focus = self.main_window_focus.next();
                    }
                    KeyCode::Right | KeyCode::Char('l') => match self.main_window_focus {
                        MainWindowFocus::MainChip => {
                            self.main_window_focus = MainWindowFocus::AgentState
                        }
                        MainWindowFocus::CompanionChip => {
                            self.main_window_focus = MainWindowFocus::DeploymentStatus
                        }
                        MainWindowFocus::SensorChip => {
                            self.main_window_focus = MainWindowFocus::DeviceReserved
                        }
                        MainWindowFocus::DeviceManifest => {
                            self.main_window_focus = MainWindowFocus::DeviceCapabilities
                        }
                        MainWindowFocus::AgentState => {
                            self.main_window_focus = MainWindowFocus::SystemSettings
                        }
                        MainWindowFocus::DeploymentStatus => {
                            self.main_window_focus = MainWindowFocus::SystemSettings
                        }
                        MainWindowFocus::DeviceReserved => {
                            self.main_window_focus = MainWindowFocus::NetworkSettings
                        }
                        MainWindowFocus::DeviceState => {
                            self.main_window_focus = MainWindowFocus::WirelessSettings
                        }
                        MainWindowFocus::DeviceCapabilities => {
                            self.main_window_focus = MainWindowFocus::WirelessSettings
                        }
                        MainWindowFocus::SystemSettings => {
                            self.main_window_focus = MainWindowFocus::MainChip
                        }
                        MainWindowFocus::NetworkSettings => {
                            self.main_window_focus = MainWindowFocus::CompanionChip
                        }
                        MainWindowFocus::WirelessSettings => {
                            self.main_window_focus = MainWindowFocus::SensorChip
                        }
                    },
                    KeyCode::Left | KeyCode::Char('h') => match self.main_window_focus {
                        MainWindowFocus::MainChip => {
                            self.main_window_focus = MainWindowFocus::SystemSettings
                        }
                        MainWindowFocus::CompanionChip => {
                            self.main_window_focus = MainWindowFocus::SystemSettings
                        }
                        MainWindowFocus::SensorChip => {
                            self.main_window_focus = MainWindowFocus::NetworkSettings
                        }
                        MainWindowFocus::DeviceManifest => {
                            self.main_window_focus = MainWindowFocus::WirelessSettings
                        }
                        MainWindowFocus::AgentState => {
                            self.main_window_focus = MainWindowFocus::MainChip
                        }
                        MainWindowFocus::DeploymentStatus => {
                            self.main_window_focus = MainWindowFocus::MainChip
                        }
                        MainWindowFocus::DeviceReserved => {
                            self.main_window_focus = MainWindowFocus::CompanionChip
                        }
                        MainWindowFocus::DeviceState => {
                            self.main_window_focus = MainWindowFocus::SensorChip
                        }
                        MainWindowFocus::DeviceCapabilities => {
                            self.main_window_focus = MainWindowFocus::DeviceManifest
                        }
                        MainWindowFocus::SystemSettings => {
                            self.main_window_focus = MainWindowFocus::AgentState
                        }
                        MainWindowFocus::NetworkSettings => {
                            self.main_window_focus = MainWindowFocus::DeploymentStatus
                        }
                        MainWindowFocus::WirelessSettings => {
                            self.main_window_focus = MainWindowFocus::DeviceState
                        }
                    },
                    KeyCode::Enter => self.dm_screen_move_to(DMScreen::Module),
                    KeyCode::Char('e') => self.switch_to_config_screen(false),
                    KeyCode::Char('E') => self.switch_to_config_screen(true),
                    KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                    KeyCode::Char('d') => self.switch_to_direct_command_screen(),
                    KeyCode::Char('m') => self.switch_to_evp_module_screen(AzuriteAction::Deploy),
                    KeyCode::Char('t') => self.switch_to_token_provider_screen(),
                    KeyCode::Char('g') => self.switch_to_elog_screen(),
                    KeyCode::Char('M') => self.switch_to_edge_app_screen(),
                    KeyCode::Char('o') => self.dm_screen_move_to(DMScreen::Ota),
                    _ => {}
                }
                // Since companion chip and sensor chip shares the same display region in main ui,
                // last_config_companion_sensor is used to remember which is the last focused.
                if self.main_window_focus == MainWindowFocus::CompanionChip
                    || self.main_window_focus == MainWindowFocus::SensorChip
                {
                    self.last_config_companion_sensor = self.main_window_focus as usize;
                }
            }

            DMScreen::Module => match key_event.code {
                KeyCode::Enter | KeyCode::Esc => self.dm_screen_move_back(),
                KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                KeyCode::Char('e') => self.switch_to_config_screen(false),
                KeyCode::Char('E') => self.switch_to_config_screen(true),
                KeyCode::Char('d') => self.switch_to_direct_command_screen(),
                KeyCode::Char('m') => self.switch_to_evp_module_screen(AzuriteAction::Deploy),
                KeyCode::Char('t') => self.switch_to_token_provider_screen(),
                KeyCode::Char('g') => self.switch_to_elog_screen(),
                KeyCode::Char('o') => self.dm_screen_move_to(DMScreen::Ota),
                _ => {}
            },

            DMScreen::Exiting => {
                match key_event.code {
                    KeyCode::Char('y') => {
                        self.exit = true;
                    }
                    KeyCode::Char('n') => {
                        self.dm_screen_move_back();
                        self.exit = false;
                    }
                    _ => {}
                };
            }
            DMScreen::ConfigurationUser => match key_event.code {
                KeyCode::Esc if self.config_result.is_some() => self.config_result = None,
                KeyCode::Char('s') => {
                    if let Some(Ok(s)) = self.config_result.as_ref() {
                        match with_mqtt_ctrl_mut(|mqtt_ctrl| mqtt_ctrl.send_configure(s)) {
                            Ok(()) => self.dm_screen_move_back(),
                            Err(_) => {
                                self.app_error = Some("Failed to send configuration!".to_owned())
                            }
                        }
                    }
                }
                KeyCode::Char('w') => match with_mqtt_ctrl(|mqtt_ctrl| {
                    mqtt_ctrl.parse_configure(None, self.main_window_focus())
                }) {
                    Ok(s) => {
                        if !s.is_empty() {
                            self.config_result = Some(Ok(s));
                        }
                    }
                    Err(e) => {
                        self.config_result = Some(Err(e));
                    }
                },
                KeyCode::Esc => self.dm_screen_move_back(),
                KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                _ => {}
            },
            DMScreen::Configuration => match key_event.code {
                KeyCode::Char(c) if self.config_key_editable => {
                    let value: &mut String =
                        self.config_keys.get_mut(self.config_key_focus).unwrap();
                    value.push(c);
                }
                KeyCode::Backspace if self.config_key_editable => {
                    let value: &mut String =
                        self.config_keys.get_mut(self.config_key_focus).unwrap();
                    value.pop();
                }
                KeyCode::Esc if self.config_key_editable => self.config_key_editable = false,
                KeyCode::Esc if self.config_result.is_some() => self.config_result = None,
                KeyCode::Esc => self.dm_screen_move_back(),
                KeyCode::Enter if self.config_key_editable => self.config_key_editable = false,
                KeyCode::Char('s') => {
                    if let Some(Ok(s)) = self.config_result.as_ref() {
                        match with_mqtt_ctrl_mut(|mqtt_ctrl| mqtt_ctrl.send_configure(s)) {
                            Ok(()) => self.dm_screen_move_back(),
                            Err(_) => {
                                self.app_error = Some("Failed to send configuration!".to_owned())
                            }
                        }
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => self.config_focus_up(),
                KeyCode::Down | KeyCode::Char('j') => self.config_focus_down(),
                KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                KeyCode::Tab => self.config_focus_down(),
                KeyCode::Char('i') | KeyCode::Char('a') => {
                    let current_config_key = ConfigKey::from(self.config_key_focus);
                    if Self::is_log_storage_config_key(current_config_key) {
                        self.token_provider_for_config = Some(current_config_key);
                        self.switch_to_token_provider_screen();
                    } else {
                        self.config_key_editable = true;
                    }
                }
                //Previous screen is used to judge what to be configured.
                KeyCode::Char('w') => match with_mqtt_ctrl(|mqtt_ctrl| {
                    mqtt_ctrl.parse_configure(Some(&self.config_keys), self.main_window_focus())
                }) {
                    Ok(s) => {
                        if !s.is_empty() {
                            self.config_result = Some(Ok(s));
                        }
                    }
                    Err(e) => {
                        self.config_result = Some(Err(e));
                    }
                },
                _ => {}
            },

            DMScreen::DirectCommand => {
                let command = with_mqtt_ctrl_mut(|mqtt_ctrl| mqtt_ctrl.get_direct_command());
                match command {
                    Some(DirectCommand::GetDirectImage) => {
                        let has_request = with_mqtt_ctrl(|mqtt_ctrl| {
                            mqtt_ctrl.direct_command_request().is_some()
                        });

                        if !has_request {
                            match key_event.code {
                                KeyCode::Char(c) if self.config_key_editable => {
                                    let value: &mut String =
                                        self.config_keys.get_mut(self.config_key_focus).unwrap();
                                    value.push(c);
                                }
                                KeyCode::Backspace if self.config_key_editable => {
                                    let value: &mut String =
                                        self.config_keys.get_mut(self.config_key_focus).unwrap();
                                    value.pop();
                                }
                                KeyCode::Esc if self.config_key_editable => {
                                    self.config_key_editable = false
                                }
                                KeyCode::Esc => self.dm_screen_move_back(),
                                KeyCode::Enter if self.config_key_editable => {
                                    self.config_key_editable = false
                                }
                                KeyCode::Tab => self.config_focus_down(),
                                KeyCode::Char('i') | KeyCode::Char('a') => {
                                    self.config_key_editable = true
                                }
                                KeyCode::Up | KeyCode::Char('k') => self.config_focus_up(),
                                KeyCode::Down | KeyCode::Char('j') => self.config_focus_down(),
                                KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                                KeyCode::Char('s') => with_mqtt_ctrl_mut(|mqtt_ctrl| {
                                    let _ = mqtt_ctrl.send_rpc_direct_get_image(&self.config_keys);
                                }),

                                _ => {}
                            }
                        } else {
                            match key_event.code {
                                KeyCode::Esc => self.dm_screen_move_back(),
                                KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                                KeyCode::Char('w') => match with_mqtt_ctrl_mut(|mqtt_ctrl| {
                                    mqtt_ctrl.save_direct_get_image()
                                }) {
                                    Ok(image_path) => {
                                        with_mqtt_ctrl_mut(|mqtt_ctrl| {
                                            mqtt_ctrl.info =
                                                Some(format!("Image saved to: {}", image_path))
                                        });
                                    }
                                    Err(e) => {
                                        self.app_error = Some(format!(
                                            "Failed to save preview image: {}",
                                            e.error_str().unwrap_or("Unknown error".to_owned())
                                        ));
                                    }
                                },
                                _ => {}
                            }
                        }
                    }
                    None => {
                        jdebug!(
                            func = "App::handle_key_event()",
                            screen = "DirectCommand",
                            line = line!()
                        );
                        match key_event.code {
                            KeyCode::Char('r') => {
                                jdebug!(func = "App::handle_key_event()", event = "Set Reboot",);
                                with_mqtt_ctrl_mut(|ctrl| {
                                    ctrl.set_direct_command(Some(DirectCommand::Reboot))
                                })
                            }
                            KeyCode::Char('i') => {
                                jdebug!(
                                    func = "App::handle_key_event()",
                                    event = "Set DirectGetImage",
                                );
                                with_mqtt_ctrl_mut(|ctrl| {
                                    ctrl.set_direct_command(Some(DirectCommand::GetDirectImage))
                                });
                                self.config_key_focus_start =
                                    ConfigKey::DirectGetImageSensorName.into();
                                self.config_key_focus_end =
                                    ConfigKey::DirectGetImageNetworkId.into();
                                self.config_key_focus = self.config_key_focus_start;
                            }
                            KeyCode::Char('f') => {
                                jdebug!(
                                    func = "App::handle_key_event()",
                                    event = "Set FactoryReset",
                                );
                                with_mqtt_ctrl_mut(|ctrl| {
                                    ctrl.set_direct_command(Some(DirectCommand::FactoryReset))
                                });
                            }
                            KeyCode::Esc => self.dm_screen_move_back(),
                            KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),

                            _ => {}
                        }
                    }
                    _ => match key_event.code {
                        KeyCode::Esc => self.dm_screen_move_back(),
                        KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                        _ => {}
                    },
                }
            }

            DMScreen::Elog => match key_event.code {
                KeyCode::Esc => self.dm_screen_move_back(),
                KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),

                KeyCode::Char('w') => {
                    match with_mqtt_ctrl_mut(|mqtt_ctrl| mqtt_ctrl.save_elogs()) {
                        Ok(elog_path) => {
                            with_mqtt_ctrl_mut(|mqtt_ctrl| {
                                mqtt_ctrl.info = Some(format!("Elog saved to: {}", elog_path))
                            });
                        }
                        Err(e) => {
                            self.app_error =
                                Some(e.error_str().unwrap_or("Unknown error".to_owned()));
                        }
                    }
                }
                _ => {}
            },

            DMScreen::EvpModule => match key_event.code {
                KeyCode::Char(c)
                    if with_azurite_storage(|storage| {
                        storage.action() == Some(AzuriteAction::Add)
                    })
                    .unwrap_or(false) =>
                {
                    with_azurite_storage_mut(|azurite_storage| {
                        azurite_storage.new_module_mut().push(c);
                    });
                }

                KeyCode::Esc
                    if with_azurite_storage(|storage| {
                        storage.action() == Some(AzuriteAction::Add)
                    })
                    .unwrap_or(false) =>
                {
                    with_azurite_storage_mut(|azurite_storage| {
                        azurite_storage.pop_action();
                        azurite_storage.new_module_mut().clear();
                    });
                }

                KeyCode::Backspace
                    if with_azurite_storage(|storage| {
                        storage.action() == Some(AzuriteAction::Add)
                    })
                    .unwrap_or(false) =>
                {
                    with_azurite_storage_mut(|azurite_storage| {
                        azurite_storage.new_module_mut().pop();
                    });
                }

                KeyCode::Enter
                    if with_azurite_storage(|storage| {
                        storage.action() == Some(AzuriteAction::Add)
                    })
                    .unwrap_or(false) =>
                {
                    if let Some((_new_module_path, push_result)) =
                        with_azurite_storage_mut(|azurite_storage| {
                            let new_module_path = azurite_storage.new_module().to_owned();
                            let push_result = azurite_storage.push_blob(None, &new_module_path);
                            (new_module_path, push_result)
                        })
                    {
                        if let Err(e) = push_result {
                            self.app_error = Some(format!(
                                "Failed to add new module: {}",
                                e.error_str().unwrap_or("Unknown error".to_owned())
                            ));
                        } else {
                            with_azurite_storage_mut(|azurite_storage| {
                                azurite_storage.update_modules(None).unwrap_or_else(|e| {
                                    // Can't set app_error from here, so just log it
                                    jerror!("Failed to update modules: {}", e);
                                });
                                azurite_storage.pop_action();
                                azurite_storage.new_module_mut().clear();
                            });
                        }
                    }
                }

                KeyCode::Enter => {
                    if with_azurite_storage(|storage| {
                        storage.action() == Some(AzuriteAction::Select)
                    })
                    .unwrap_or(false)
                    {
                        self.update_sas_url_entries();
                        self.dm_screen_move_back();
                    }
                }

                KeyCode::Char('a') => {
                    with_azurite_storage_mut(|azurite_storage| {
                        azurite_storage.push_action(AzuriteAction::Add);
                    });
                }

                KeyCode::Char('r') => {
                    if let Some(module_name) = with_azurite_storage(|azurite_storage| {
                        azurite_storage
                            .current_module()
                            .map(|m| m.blob_name.clone())
                    })
                    .flatten()
                    {
                        let remove_result = with_azurite_storage_mut(|azurite_storage| {
                            azurite_storage.remove_blob(None, &module_name)
                        });

                        if let Some(Err(e)) = remove_result {
                            self.app_error = Some(format!(
                                "Failed to remove module '{}': {}",
                                module_name,
                                e.error_str().unwrap_or("Unknown error".to_owned())
                            ));
                        } else {
                            with_azurite_storage_mut(|azurite_storage| {
                                azurite_storage.update_modules(None).unwrap_or_else(|e| {
                                    jerror!("Failed to update modules: {}", e);
                                });
                            });
                        }
                    }
                }

                KeyCode::Esc if self.config_result.is_some() => self.config_result = None,
                KeyCode::Char('d')
                    if with_azurite_storage(|az| az.action() == Some(AzuriteAction::Deploy))
                        .unwrap_or(false) =>
                {
                    if with_mqtt_ctrl(|mqtt_ctrl| mqtt_ctrl.is_device_connected()) {
                        if let Some(deployment_json) = with_azurite_storage(|azurite_storage| {
                            azurite_storage
                                .current_module()
                                .map(|m| m.deployment_json())
                        })
                        .flatten()
                        {
                            self.config_result = Some(deployment_json);
                        }
                    } else {
                        self.app_error = Some("Device is not connected.".to_owned());
                    }
                }

                KeyCode::Char('u')
                    if with_azurite_storage(|az| az.action() == Some(AzuriteAction::Deploy))
                        .unwrap_or(false) =>
                {
                    if with_mqtt_ctrl(|mqtt_ctrl| mqtt_ctrl.is_device_connected()) {
                        self.config_result = Some(ModuleInfo::undeployment_json());
                    } else {
                        self.app_error = Some("Device is not connected.".to_owned());
                    }
                }

                KeyCode::Char('s') => {
                    if with_mqtt_ctrl(|mqtt_ctrl| mqtt_ctrl.is_device_connected()) {
                        if let Some(Ok(deploy)) = &self.config_result {
                            match with_mqtt_ctrl_mut(|mqtt_ctrl| mqtt_ctrl.send_configure(deploy)) {
                                Ok(()) => self.dm_screen_move_back(),
                                Err(_) => {
                                    self.app_error = Some("Failed to send deployment.".to_owned());
                                }
                            }
                        }
                    } else {
                        self.app_error = Some("Device is not connected.".to_owned());
                    }
                }

                KeyCode::Esc => self.dm_screen_move_back(),
                KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                KeyCode::Up | KeyCode::Char('k') => {
                    with_azurite_storage_mut(|azurite_storage| {
                        azurite_storage.current_module_focus_up();
                    });
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    with_azurite_storage_mut(|azurite_storage| {
                        azurite_storage.current_module_focus_down();
                    });
                }
                _ => {}
            },
            DMScreen::TokenProvider => match key_event.code {
                KeyCode::Enter if self.token_provider_for_config.is_some() => {
                    if let Some(uuid_string) = with_azurite_storage(|azurite_storage| {
                        azurite_storage
                            .current_token_provider()
                            .map(|tp| tp.uuid.uuid().to_string())
                    })
                    .flatten()
                    {
                        if let Some(config_key) = self.token_provider_for_config.take() {
                            self.config_keys[usize::from(config_key)] = uuid_string;
                            self.dm_screen_move_back();
                        }
                    }
                }
                KeyCode::Char('a') => {
                    if let Some(result) = with_azurite_storage_mut(|azurite_storage| {
                        azurite_storage.add_token_provider()
                    }) {
                        if let Err(e) = result {
                            self.app_error = Some(format!(
                                "Failed to add new token provider: {}",
                                e.error_str().unwrap_or("Unknown error".to_owned())
                            ));
                        }
                    }
                }
                KeyCode::Char('d') => {
                    if let Some(uuid) = with_azurite_storage(|azurite_storage| {
                        azurite_storage
                            .current_token_provider()
                            .map(|tp| tp.uuid.clone())
                    })
                    .flatten()
                    {
                        if let Some(result) = with_azurite_storage_mut(|azurite_storage| {
                            azurite_storage.remove_token_provider(&uuid)
                        }) {
                            if let Err(e) = result {
                                self.app_error = Some(format!(
                                    "Failed to remove token provider: {}",
                                    e.error_str().unwrap_or("Unknown error".to_owned())
                                ));
                            }
                        }
                    }
                }
                KeyCode::Esc => {
                    self.token_provider_for_config = None;
                    self.dm_screen_move_back();
                }
                KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                KeyCode::Up | KeyCode::Char('k') => {
                    with_azurite_storage_mut(|azurite_storage| {
                        azurite_storage.current_token_provider_focus_up();
                    });
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    with_azurite_storage_mut(|azurite_storage| {
                        azurite_storage.current_token_provider_focus_down();
                    });
                }
                KeyCode::Char('s') => {
                    if let Some(token_provider) = with_azurite_storage(|azurite_storage| {
                        azurite_storage.current_token_provider().cloned()
                    })
                    .flatten()
                    {
                        let container_name = token_provider.container.clone();

                        // Fetch blobs for the selected token provider
                        match with_azurite_storage(|azurite_storage| {
                            azurite_storage.list_blobs_for_ui(&container_name)
                        }) {
                            Some(Ok(blobs)) => {
                                let mut blob_state =
                                    ui::ui_token_provider_blobs::BlobListState::new(container_name);
                                blob_state.blobs = blobs;
                                self.blob_list_state = Some(blob_state);
                                self.dm_screen_move_to(DMScreen::TokenProviderBlobs);
                            }
                            Some(Err(e)) => {
                                self.app_error = Some(format!(
                                    "Failed to list blobs: {}",
                                    e.error_str().unwrap_or("Unknown error".to_owned())
                                ));
                            }
                            None => {
                                self.app_error = Some("Azurite storage not available".to_owned());
                            }
                        }
                    }
                }
                _ => {}
            },
            DMScreen::TokenProviderBlobs => match key_event.code {
                KeyCode::Esc => {
                    self.blob_list_state = None;
                    self.dm_screen_move_back();
                }
                KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                KeyCode::Up | KeyCode::Char('k') => {
                    if let Some(ref mut blob_state) = self.blob_list_state {
                        blob_state.move_up();
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if let Some(ref mut blob_state) = self.blob_list_state {
                        blob_state.move_down();
                    }
                }
                KeyCode::Enter => {
                    if let Some(ref blob_state) = self.blob_list_state {
                        if let Some(blob) = blob_state.current_blob() {
                            let container_name = blob_state.container_name.clone();
                            let blob_name = blob.name.clone();

                            match with_azurite_storage(|azurite_storage| {
                                azurite_storage
                                    .download_blob_to_current_dir(&container_name, &blob_name)
                            }) {
                                Some(Ok(file_path)) => {
                                    with_mqtt_ctrl_mut(|mqtt_ctrl| {
                                        mqtt_ctrl.info =
                                            Some(format!("Blob downloaded to: {}", file_path));
                                    });
                                }
                                Some(Err(e)) => {
                                    self.app_error =
                                        Some(e.error_str().unwrap_or("Unknown error".to_owned()));
                                }
                                None => {
                                    self.app_error =
                                        Some("Azurite storage not available".to_owned());
                                }
                            }
                        }
                    }
                }
                _ => {}
            },
            DMScreen::EdgeApp(state) => match state {
                DMScreenState::Initial => match key_event.code {
                    KeyCode::Esc => self.dm_screen_move_back(),
                    KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                    KeyCode::Char('e') => {
                        self.config_key_focus_start = ConfigKey::CommonSettingsProcessState.into();
                        self.config_key_focus_end = ConfigKey::CommonSettingsUploadInterval.into();
                        self.config_key_focus = self.config_key_focus_start;
                        self.dm_screen_move_to(DMScreen::EdgeApp(DMScreenState::Configuring));
                    }
                    _ => {}
                },
                DMScreenState::Configuring => match key_event.code {
                    KeyCode::Char(c) if self.config_key_editable => {
                        let value: &mut String =
                            self.config_keys.get_mut(self.config_key_focus).unwrap();
                        value.push(c);
                    }
                    KeyCode::Backspace if self.config_key_editable => {
                        let value: &mut String =
                            self.config_keys.get_mut(self.config_key_focus).unwrap();
                        value.pop();
                    }
                    KeyCode::Esc | KeyCode::Enter if self.config_key_editable => {
                        self.config_key_editable = false
                    }
                    KeyCode::Esc if self.config_result.is_some() => self.config_result = None,
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.config_focus_up();
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.config_focus_down();
                    }
                    KeyCode::Char('w') => {
                        let mut edge_app_result = None;

                        with_mqtt_ctrl(|ctrl| {
                            if let Some(edge_app) = ctrl.edge_app() {
                                edge_app_result = Some(edge_app.parse_configure(&self.config_keys));
                            }
                        });

                        if let Some(result) = edge_app_result {
                            self.config_result = match result {
                                Ok(s) => Some(Ok(s)),
                                Err(e) => Some(Err(e)),
                            };
                            self.dm_screen_move_to(DMScreen::EdgeApp(DMScreenState::Completed));
                        } else {
                            self.app_error = Some("No Edge App instances found.".to_owned());
                            self.dm_screen_move_back();
                        }
                    }
                    KeyCode::Char('i') | KeyCode::Char('a') => self.config_key_editable = true,
                    KeyCode::Esc => self.dm_screen_move_back(),
                    KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                    _ => {}
                },
                DMScreenState::Completed => match key_event.code {
                    KeyCode::Char('s') => {
                        // Send the configuration, go back to the default state
                        self.config_key_clear();
                        self.dm_screen_move_back();
                        self.dm_screen_move_back();
                    }
                    KeyCode::Esc => self.dm_screen_move_back(),
                    KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                    _ => {}
                },
            },
            DMScreen::Ota => match key_event.code {
                KeyCode::Esc => self.dm_screen_move_back(),
                KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                KeyCode::Char('d') => {
                    let is_device_connected =
                        with_mqtt_ctrl(|mqtt_ctrl| mqtt_ctrl.is_device_connected());
                    if is_device_connected {
                        self.switch_to_ota_config_screen(DMScreenState::Initial);
                    } else {
                        self.app_error = Some("Device is not connected.".to_owned());
                    }
                }
                _ => {}
            },
            DMScreen::OtaConfig(state) => match state {
                DMScreenState::Initial => match key_event.code {
                    KeyCode::Char(c) if self.config_key_editable => {
                        let value: &mut String =
                            self.config_keys.get_mut(self.config_key_focus).unwrap();
                        value.push(c);
                    }
                    KeyCode::Backspace if self.config_key_editable => {
                        let value: &mut String =
                            self.config_keys.get_mut(self.config_key_focus).unwrap();
                        value.pop();
                    }
                    KeyCode::Esc if self.config_key_editable => self.config_key_editable = false,
                    KeyCode::Esc => self.dm_screen_move_back(),
                    KeyCode::Enter if self.config_key_editable => self.config_key_editable = false,
                    KeyCode::Up | KeyCode::Char('k') => self.config_focus_up(),
                    KeyCode::Down | KeyCode::Char('j') => self.config_focus_down(),
                    KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                    KeyCode::Char('i') | KeyCode::Char('a') => {
                        if ConfigKey::from(self.config_key_focus).is_sas_url_entry() {
                            self.switch_to_evp_module_screen(AzuriteAction::Select);
                        } else {
                            self.config_key_editable = true;
                        }
                    }
                    KeyCode::Char('w') => {
                        self.dm_screen_move_to(DMScreen::OtaConfig(DMScreenState::Configuring));
                    }
                    _ => {}
                },
                DMScreenState::Configuring => {
                    // Handle configuration logic here
                }
                DMScreenState::Completed => {
                    // Handle completion logic here
                }
            },
        }
    }

    pub fn should_exit(&self) -> bool {
        self.exit
    }

    pub fn main_window_focus(&self) -> MainWindowFocus {
        self.main_window_focus
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let draw_start = Instant::now();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(30),
                Constraint::Length(1),
            ])
            .split(area);

        if let Err(e) = ui_head::draw(chunks[0], buf, self) {
            jerror!(func = "App::render()", error = format!("{:?}", e));
        }

        // Draw main content based on current screen
        match self.current_screen() {
            DMScreen::Main => {
                if let Err(e) = ui_main::draw(chunks[1], buf, self) {
                    jerror!(func = "App::render()", error = format!("{:?}", e));
                }
                jinfo!(
                    event = "TIME_MEASURE",
                    draw_main_time = format!("{}ms", draw_start.elapsed().as_millis())
                )
            }
            DMScreen::Module => {
                if let Err(e) = ui_module::draw(chunks[1], buf, self) {
                    jerror!(func = "App::render()", error = format!("{:?}", e));
                }
                jinfo!(
                    event = "TIME_MEASURE",
                    draw_module_time = format!("{}ms", draw_start.elapsed().as_millis())
                )
            }
            DMScreen::Configuration => {
                if let Err(e) = ui_config::draw(chunks[1], buf, self) {
                    jerror!(func = "App::render()", error = format!("{:?}", e));
                }
            }
            DMScreen::ConfigurationUser => {
                if let Err(e) = ui_config_user::draw(chunks[1], buf, self) {
                    jerror!(func = "App::render()", error = format!("{:?}", e));
                }
            }
            DMScreen::DirectCommand => {
                if let Err(e) = ui_directcmd::draw(chunks[1], buf, self) {
                    jerror!(func = "App::render()", error = format!("{:?}", e));
                }
            }
            DMScreen::EvpModule => {
                if let Err(e) = ui_deploy::draw(chunks[1], buf, self) {
                    jerror!(func = "App::render()", error = format!("{:?}", e));
                }
            }
            DMScreen::TokenProvider => {
                if let Err(e) = ui_token_provider::draw(chunks[1], buf, self) {
                    jerror!(func = "App::render()", error = format!("{:?}", e));
                }
            }
            DMScreen::TokenProviderBlobs => {
                if let Some(ref blob_state) = self.blob_list_state {
                    if let Err(e) = ui::ui_token_provider_blobs::draw(chunks[1], buf, blob_state) {
                        jerror!(func = "App::render()", error = format!("{:?}", e));
                    }
                }
            }
            DMScreen::Elog => {
                if let Err(e) = ui_elog::draw(chunks[1], buf, self) {
                    jerror!(func = "App::render()", error = format!("{:?}", e));
                }
            }
            DMScreen::EdgeApp(_) => {
                if let Err(e) = ui_edge_app::draw(chunks[1], buf, self) {
                    jerror!(func = "App::render()", error = format!("{:?}", e));
                }
            }
            DMScreen::Ota => {
                if let Err(e) = ui::ui_ota::draw(chunks[1], buf, self) {
                    jerror!(func = "App::render()", error = format!("{:?}", e));
                }
            }
            DMScreen::Exiting => {
                if let Err(e) = ui_exit::draw(chunks[1], buf, self) {
                    jerror!(func = "App::render()", error = format!("{:?}", e));
                }
                jinfo!(
                    event = "TIME_MEASURE",
                    draw_exit_time = format!("{}ms", draw_start.elapsed().as_millis())
                )
            }

            DMScreen::OtaConfig(_) => {
                if let Err(e) = ui_ota_config::draw(chunks[1], buf, self) {
                    jerror!(func = "App::render()", error = format!("{:?}", e));
                }
            }
        }

        if let Err(e) = ui_foot::draw(chunks[2], buf, self) {
            jerror!(func = "App::render()", error = format!("{:?}", e));
        }
    }
}

// Module-level functions that operate on the global App instance

/// Handle terminal events using the global App instance
pub fn handle_events() -> Result<(), DMError> {
    with_global_app_mut(|app| {
        let has_new_event = event::poll(Duration::from_millis(DEFAULT_EVENT_POLL_TIMEOUT))
            .map_err(|e| Report::new(DMError::IOError).attach_printable(e))?;

        if has_new_event {
            let event = event::read().map_err(|_| Report::new(DMError::IOError))?;
            match event {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    app.handle_key_event(key_event)
                }
                _ => {}
            }
        }

        Ok(())
    })
}

/// Update the global App instance
pub fn update() -> Result<(), DMError> {
    with_global_app_mut(|app| {
        if let Err(e) = with_mqtt_ctrl_mut(|mqtt_ctrl| mqtt_ctrl.update()) {
            jerror!(func = "update()", error = format!("{:?}", e));
            app.app_error = Some(e.error_str().unwrap_or("Update error!".to_owned()));
        }

        // Try to reinitialize AzuriteStorage if it's currently None
        if with_azurite_storage(|_| true).is_none() {
            if try_reinit_azurite_storage() {
                jinfo!("AzuriteStorage successfully reinitialized during update cycle");
            }
        }

        Ok(())
    })
}

/// Draw the global App instance to a terminal frame
pub fn draw(frame: &mut Frame) {
    with_global_app(|app| {
        frame.render_widget(app, frame.area());
    })
}

/// Check if the global App should exit
pub fn should_exit() -> bool {
    with_global_app(|app| app.should_exit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_config_key_from_usize_conversion() {
        // Test valid conversions
        assert_eq!(ConfigKey::from(0), ConfigKey::ReportStatusIntervalMin);
        assert_eq!(ConfigKey::from(1), ConfigKey::ReportStatusIntervalMax);

        // Test invalid conversion returns Invalid
        let invalid_index = ConfigKey::size();
        assert_eq!(ConfigKey::from(invalid_index), ConfigKey::Invalid);
        assert_eq!(ConfigKey::from(1000), ConfigKey::Invalid);
    }

    #[test]
    fn test_config_key_size_consistency() {
        // Ensure size is consistent with Invalid variant position
        let expected_size = ConfigKey::Invalid as usize + 1;
        assert_eq!(ConfigKey::size(), expected_size);
    }

    #[test]
    fn test_main_window_focus_navigation() {
        let focus = MainWindowFocus::MainChip;

        // Test forward navigation
        let next = focus.next();
        assert_eq!(next, MainWindowFocus::CompanionChip);

        // Test backward navigation
        let prev = focus.previous();
        assert_eq!(prev, MainWindowFocus::WirelessSettings);

        // Test wrap-around at end
        let last = MainWindowFocus::WirelessSettings;
        assert_eq!(last.next(), MainWindowFocus::MainChip);
    }

    #[test]
    fn test_default_event_poll_timeout() {
        // Ensure the timeout constant is reasonable
        assert!(DEFAULT_EVENT_POLL_TIMEOUT > 0);
        assert!(DEFAULT_EVENT_POLL_TIMEOUT <= 1000); // Not too long
    }

    #[test]
    fn test_app_new_and_basic_properties() {
        let cfg = AppConfig {
            broker: "localhost:1883",
        };
        let app = App::new(cfg).unwrap();

        // Initial screen should be Main
        assert_eq!(app.current_screen(), DMScreen::Main);
        // Should not request exit by default
        assert!(!app.should_exit());
        // Default main window focus should be MainChip
        assert_eq!(app.main_window_focus(), MainWindowFocus::MainChip);
    }

    #[test]
    fn test_config_key_clear_and_result_reset() {
        let mut app = App::new(AppConfig { broker: "b" }).unwrap();
        // modify keys and result
        app.config_keys[0] = "value".to_string();
        app.config_result = Some(Ok("ok".to_string()));

        app.config_key_clear();

        for k in app.config_keys.iter() {
            assert!(k.is_empty());
        }
        assert!(app.config_result.is_none());
    }

    #[test]
    fn test_config_focus_navigation_wraps() {
        let mut app = App::new(AppConfig { broker: "b" }).unwrap();
        app.config_key_focus_start = 2;
        app.config_key_focus_end = 4;
        app.config_key_focus = app.config_key_focus_start;

        // Up from start should wrap to end
        app.config_focus_up();
        assert_eq!(app.config_key_focus, 4);

        // Down from end should wrap to start
        app.config_focus_down();
        assert_eq!(app.config_key_focus, app.config_key_focus_start);
    }

    #[test]
    fn test_handle_key_event_changes_focus() {
        let mut app = App::new(AppConfig { broker: "b" }).unwrap();

        // Simulate Down key press
        let down_event = KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE);
        app.handle_key_event(down_event);
        assert_eq!(app.main_window_focus(), MainWindowFocus::CompanionChip);

        // Simulate Up key press
        let up_event = KeyEvent::new(KeyCode::Up, crossterm::event::KeyModifiers::NONE);
        app.handle_key_event(up_event);
        // Up from CompanionChip goes back to MainChip
        assert_eq!(app.main_window_focus(), MainWindowFocus::MainChip);
    }

    #[test]
    #[serial]
    fn test_config_dir_prefers_env_var() {
        // Use serial attribute to prevent parallel test interference when mutating process-global env.
        // Call env APIs inside unsafe blocks in case the platform marks them as requiring unsafe.
        unsafe {
            std::env::set_var("DM_CONFIG_DIR", "/tmp/testcfg_app");
        }
        let dir = App::config_dir();
        assert_eq!(dir, "/tmp/testcfg_app");
        unsafe {
            std::env::remove_var("DM_CONFIG_DIR");
        }
    }
}
