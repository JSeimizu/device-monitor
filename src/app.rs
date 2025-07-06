mod ui;

#[allow(unused)]
use {
    super::{
        app,
        azurite::{AzuriteAction, AzuriteStorage},
        error::{DMError, DMErrorExt},
        mqtt_ctrl::MqttCtrl,
        mqtt_ctrl::evp::module::ModuleInfo,
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
        time::{Duration, Instant},
    },
    ui::*,
};

pub struct AppConfig<'a> {
    pub broker: &'a str,
    pub azurite_url: &'a str,
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub enum DMScreenState {
    #[default]
    DefaultState,

    ConfigureState,
    ResultState,
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub enum DMScreen {
    #[default]
    Main,
    Module,
    Configuration,
    ConfigurationUser,
    DirectCommand,
    EvpModule,
    Elog,
    EdgeApp(DMScreenState),
    Exiting,
}

#[derive(Debug, Default, PartialEq, PartialOrd, Clone, Copy)]
#[repr(usize)]
#[allow(unused)]
pub enum DirectCommand {
    Reboot = 0,
    GetDirectImage,
    FactoryReset,
    ReadSensorRegister,
    WriteSensorRegister,
    ShutDown,

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

        for i in 0..ConfigKey::size() {
            if value == i {
                // SAFETY: The value is guaranteed to be a valid ConfigKey
                return unsafe { std::mem::transmute::<usize, app::ConfigKey>(i) };
            }
        }

        ConfigKey::Invalid
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

            _ => "",
        }
    }
}

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

pub struct App {
    exit: bool,
    mqtt_ctrl: MqttCtrl,
    screens: Vec<DMScreen>,
    main_window_focus: MainWindowFocus,
    config_keys: Vec<String>,
    config_key_focus: usize,
    config_key_focus_start: usize,
    config_key_focus_end: usize,
    config_key_editable: bool,
    config_result: Option<Result<String, DMError>>,
    app_error: Option<String>,
    azurite_storage: Option<AzuriteStorage>,
}

impl App {
    pub fn new(cfg: AppConfig) -> Result<Self, DMError> {
        let broker = cfg.broker;
        let (broker_url, broker_port_str) = broker.split_once(':').unwrap_or((broker, "1883"));
        let broker_port = broker_port_str.parse().map_err(|_| {
            Report::new(DMError::InvalidData)
                .attach_printable(format!("Invalid broker port: {}", broker_port_str))
        })?;

        let mqtt_ctrl = MqttCtrl::new(broker_url, broker_port)?;
        let azurite_storage = AzuriteStorage::new(cfg.azurite_url).ok();

        Ok(Self {
            mqtt_ctrl,
            exit: false,
            screens: vec![DMScreen::Main],
            main_window_focus: MainWindowFocus::default(),
            // Initialize config keys with empty strings excluding the invalid key
            config_keys: (0..ConfigKey::size()).map(|_| String::new()).collect(),
            config_key_focus: 0,
            config_key_focus_start: 0,
            config_key_focus_end: 0,
            config_key_editable: false,
            config_result: None,
            app_error: None,
            azurite_storage,
        })
    }

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
        self.mqtt_ctrl.info = None;
    }

    pub fn dm_screen_move_back(&mut self) {
        if self.screens.len() > 1 {
            self.screens.pop();
        }

        self.app_error = None;
        self.mqtt_ctrl.info = None;

        // Clear the config keys and ModuleInfo when moving back to Main
        match self.current_screen() {
            DMScreen::Main | DMScreen::Module => {
                self.config_key_clear();
                self.mqtt_ctrl.direct_command_clear();
                if let Some(azurite_storage) = &mut self.azurite_storage {
                    azurite_storage.current_module_focus_init();
                    azurite_storage.set_action(AzuriteAction::Deploy);
                }
            }
            _ => {}
        }
    }

    pub fn update(&mut self) -> Result<(), DMError> {
        if let Err(e) = self.mqtt_ctrl.update() {
            jerror!(func = "App::update()", error = format!("{:?}", e));
            self.app_error = Some(e.error_str().unwrap_or("Update error!".to_owned()));
        }

        Ok(())
    }

    pub fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    pub fn handle_events(&mut self) -> Result<(), DMError> {
        let has_new_event = event::poll(Duration::from_millis(250))
            .map_err(|e| Report::new(DMError::IOError).attach_printable(e))?;

        if has_new_event {
            let event = event::read().map_err(|_| Report::new(DMError::IOError))?;
            match event {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    self.handle_key_event(key_event)
                }
                _ => {}
            }
        }

        Ok(())
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

    pub fn config_key_clear(&mut self) {
        self.config_keys = (0..ConfigKey::size() - 1).map(|_| String::new()).collect();
        self.config_result = None;
    }

    pub fn switch_to_evp_module_screen(&mut self) {
        // Retrieve module information from Azurite storage when moving to EvpModule screen
        if let Some(azurite_storage) = &mut self.azurite_storage {
            if let Err(e) = azurite_storage.update_modules(None) {
                self.app_error = Some(format!(
                    "Failed to update modules from Azurite: {}",
                    e.error_str().unwrap_or("Unknown error".to_owned())
                ));
            } else {
                azurite_storage.current_module_focus_init();
                self.dm_screen_move_to(DMScreen::EvpModule);
            }
        }
    }

    pub fn switch_to_edge_app_screen(&mut self) {
        if let Some(status) = self.mqtt_ctrl.deployment_status() {
            if !status.instances().is_empty() {
                self.dm_screen_move_to(DMScreen::EdgeApp(DMScreenState::DefaultState));
            } else {
                self.app_error = Some("No Edge App instances found.".to_owned());
            }
        }
    }

    pub fn switch_to_elog_screen(&mut self) {
        if self.mqtt_ctrl.is_device_connected() {
            self.dm_screen_move_to(DMScreen::Elog);
        } else {
            self.app_error = Some("Device is not connected.".to_owned());
        }
    }

    pub fn switch_to_direct_command_screen(&mut self) {
        if self.mqtt_ctrl.is_device_connected() {
            self.config_key_clear();
            self.mqtt_ctrl.direct_command_clear();
            self.dm_screen_move_to(DMScreen::DirectCommand);
        } else {
            self.app_error = Some("Device is not connected.".to_owned());
        }
    }

    pub fn switch_to_config_screen(&mut self, user_config: bool) {
        if self.mqtt_ctrl.is_device_connected() {
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

    pub fn handle_key_event(&mut self, key_event: KeyEvent) {
        match self.current_screen() {
            DMScreen::Main => match key_event.code {
                KeyCode::Up | KeyCode::Char('k') => match self.main_window_focus {
                    MainWindowFocus::MainChip => {
                        self.main_window_focus = MainWindowFocus::WirelessSettings
                    }
                    MainWindowFocus::CompanionChip => {
                        self.main_window_focus = MainWindowFocus::MainChip
                    }
                    MainWindowFocus::SensorChip => {
                        self.main_window_focus = MainWindowFocus::CompanionChip
                    }
                    MainWindowFocus::DeviceManifest => {
                        self.main_window_focus = MainWindowFocus::SensorChip
                    }
                    MainWindowFocus::AgentState => {
                        self.main_window_focus = MainWindowFocus::DeviceManifest
                    }
                    MainWindowFocus::DeploymentStatus => {
                        self.main_window_focus = MainWindowFocus::AgentState
                    }
                    MainWindowFocus::DeviceReserved => {
                        self.main_window_focus = MainWindowFocus::DeploymentStatus
                    }
                    MainWindowFocus::DeviceState => {
                        self.main_window_focus = MainWindowFocus::DeviceReserved
                    }
                    MainWindowFocus::DeviceCapabilities => {
                        self.main_window_focus = MainWindowFocus::DeviceState
                    }
                    MainWindowFocus::SystemSettings => {
                        self.main_window_focus = MainWindowFocus::DeviceCapabilities
                    }
                    MainWindowFocus::NetworkSettings => {
                        self.main_window_focus = MainWindowFocus::SystemSettings
                    }
                    MainWindowFocus::WirelessSettings => {
                        self.main_window_focus = MainWindowFocus::NetworkSettings
                    }
                },
                KeyCode::Down | KeyCode::Char('j') => match self.main_window_focus {
                    MainWindowFocus::MainChip => {
                        self.main_window_focus = MainWindowFocus::CompanionChip
                    }
                    MainWindowFocus::CompanionChip => {
                        self.main_window_focus = MainWindowFocus::SensorChip
                    }
                    MainWindowFocus::SensorChip => {
                        self.main_window_focus = MainWindowFocus::DeviceManifest
                    }
                    MainWindowFocus::DeviceManifest => {
                        self.main_window_focus = MainWindowFocus::AgentState
                    }
                    MainWindowFocus::AgentState => {
                        self.main_window_focus = MainWindowFocus::DeploymentStatus
                    }
                    MainWindowFocus::DeploymentStatus => {
                        self.main_window_focus = MainWindowFocus::DeviceReserved
                    }
                    MainWindowFocus::DeviceReserved => {
                        self.main_window_focus = MainWindowFocus::DeviceState
                    }
                    MainWindowFocus::DeviceState => {
                        self.main_window_focus = MainWindowFocus::DeviceCapabilities
                    }
                    MainWindowFocus::DeviceCapabilities => {
                        self.main_window_focus = MainWindowFocus::SystemSettings
                    }
                    MainWindowFocus::SystemSettings => {
                        self.main_window_focus = MainWindowFocus::NetworkSettings
                    }
                    MainWindowFocus::NetworkSettings => {
                        self.main_window_focus = MainWindowFocus::WirelessSettings
                    }
                    MainWindowFocus::WirelessSettings => {
                        self.main_window_focus = MainWindowFocus::MainChip
                    }
                },
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
                KeyCode::Char('m') => self.switch_to_evp_module_screen(),
                KeyCode::Char('g') => self.switch_to_elog_screen(),
                KeyCode::Char('M') => self.switch_to_edge_app_screen(),
                _ => {}
            },

            DMScreen::Module => match key_event.code {
                KeyCode::Enter | KeyCode::Esc => self.dm_screen_move_back(),
                KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                KeyCode::Char('e') => self.switch_to_config_screen(false),
                KeyCode::Char('E') => self.switch_to_config_screen(true),
                KeyCode::Char('d') => self.switch_to_direct_command_screen(),
                KeyCode::Char('m') => self.switch_to_evp_module_screen(),
                KeyCode::Char('g') => self.switch_to_elog_screen(),
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
                        match self.mqtt_ctrl.send_configure(s) {
                            Ok(()) => self.dm_screen_move_back(),
                            Err(_) => {
                                self.app_error = Some("Failed to send configuration!".to_owned())
                            }
                        }
                    }
                }
                KeyCode::Char('w') => match self
                    .mqtt_ctrl
                    .parse_configure(None, self.main_window_focus())
                {
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
                        match self.mqtt_ctrl.send_configure(s) {
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
                KeyCode::Char('i') | KeyCode::Char('a') => self.config_key_editable = true,
                //Previous screen is used to judge what to be configured.
                KeyCode::Char('w') => match self
                    .mqtt_ctrl
                    .parse_configure(Some(&self.config_keys), self.main_window_focus())
                {
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

            DMScreen::DirectCommand => match self.mqtt_ctrl.get_direct_command() {
                Some(DirectCommand::GetDirectImage) => {
                    if self.mqtt_ctrl.direct_command_request().is_none() {
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
                            KeyCode::Char('s') => {
                                let _ = self.mqtt_ctrl.send_rpc_direct_get_image(&self.config_keys);
                            }
                            _ => {}
                        }
                    } else {
                        match key_event.code {
                            KeyCode::Esc => self.dm_screen_move_back(),
                            KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                            KeyCode::Char('w') => match self.mqtt_ctrl.save_direct_get_image() {
                                Ok(image_path) => {
                                    self.mqtt_ctrl.info =
                                        Some(format!("Image saved to: {}", image_path));
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
                            self.mqtt_ctrl
                                .set_direct_command(Some(DirectCommand::Reboot));
                        }
                        KeyCode::Char('i') => {
                            jdebug!(
                                func = "App::handle_key_event()",
                                event = "Set DirectGetImage",
                            );
                            self.mqtt_ctrl
                                .set_direct_command(Some(DirectCommand::GetDirectImage));
                            self.config_key_focus_start =
                                ConfigKey::DirectGetImageSensorName.into();
                            self.config_key_focus_end = ConfigKey::DirectGetImageNetworkId.into();
                            self.config_key_focus = self.config_key_focus_start;
                        }
                        KeyCode::Char('f') => {
                            jdebug!(func = "App::handle_key_event()", event = "Set FactoryReset",);
                            self.mqtt_ctrl
                                .set_direct_command(Some(DirectCommand::FactoryReset));
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
            },

            DMScreen::Elog => match key_event.code {
                KeyCode::Esc => self.dm_screen_move_back(),
                KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),

                KeyCode::Char('w') => match self.mqtt_ctrl.save_elogs() {
                    Ok(elog_path) => {
                        self.mqtt_ctrl.info = Some(format!("Elog saved to: {}", elog_path));
                    }
                    Err(e) => {
                        self.app_error = Some(e.error_str().unwrap_or("Unknown error".to_owned()));
                    }
                },
                _ => {}
            },

            DMScreen::EvpModule => match key_event.code {
                KeyCode::Char(c)
                    if self.azurite_storage.is_some()
                        && self.azurite_storage.as_ref().unwrap().action()
                            == AzuriteAction::Add =>
                {
                    self.azurite_storage
                        .as_mut()
                        .unwrap()
                        .new_module_mut()
                        .push(c);
                }

                KeyCode::Esc
                    if self.azurite_storage.is_some()
                        && self.azurite_storage.as_ref().unwrap().action()
                            == AzuriteAction::Add =>
                {
                    self.azurite_storage
                        .as_mut()
                        .unwrap()
                        .set_action(AzuriteAction::Deploy);
                    self.azurite_storage
                        .as_mut()
                        .unwrap()
                        .new_module_mut()
                        .clear();
                }

                KeyCode::Backspace
                    if self.azurite_storage.is_some()
                        && self.azurite_storage.as_ref().unwrap().action()
                            == AzuriteAction::Add =>
                {
                    self.azurite_storage
                        .as_mut()
                        .unwrap()
                        .new_module_mut()
                        .pop();
                }

                KeyCode::Enter
                    if self.azurite_storage.is_some()
                        && self.azurite_storage.as_ref().unwrap().action()
                            == AzuriteAction::Add =>
                {
                    let azurite_storage = self.azurite_storage.as_mut().unwrap();
                    let new_module_path = azurite_storage.new_module().to_owned();

                    if let Err(e) = azurite_storage.push_blob(None, &new_module_path) {
                        self.app_error = Some(format!(
                            "Failed to add new module: {}",
                            e.error_str().unwrap_or("Unknown error".to_owned())
                        ));
                    } else {
                        azurite_storage.update_modules(None).unwrap_or_else(|e| {
                            self.app_error = Some(format!(
                                "Failed to update modules: {}",
                                e.error_str().unwrap_or("Unknown error".to_owned())
                            ));
                        });
                        azurite_storage.set_action(AzuriteAction::Deploy);
                        azurite_storage.new_module_mut().clear();
                    }
                }

                KeyCode::Char('a') => {
                    if let Some(azurite_storage) = &mut self.azurite_storage {
                        azurite_storage.set_action(AzuriteAction::Add);
                    }
                }

                KeyCode::Char('r') => {
                    if let Some(azurite_storage) = &mut self.azurite_storage {
                        if let Some(module) = azurite_storage.current_module() {
                            let name = &module.blob_name;
                            azurite_storage.remove_blob(None, name).unwrap_or_else(|e| {
                                self.app_error = Some(format!(
                                    "Failed to remove module '{}': {}",
                                    name,
                                    e.error_str().unwrap_or("Unknown error".to_owned())
                                ));
                            });
                        }

                        azurite_storage.update_modules(None).unwrap_or_else(|e| {
                            self.app_error = Some(format!(
                                "Failed to update modules: {}",
                                e.error_str().unwrap_or("Unknown error".to_owned())
                            ));
                        });
                    }
                }

                KeyCode::Esc if self.config_result.is_some() => self.config_result = None,
                KeyCode::Char('d') => {
                    if self.mqtt_ctrl.is_device_connected() {
                        if let Some(azurite_storage) = &mut self.azurite_storage {
                            if let Some(module) = azurite_storage.current_module() {
                                self.config_result = Some(module.deployment_json());
                            }
                        }
                    } else {
                        self.app_error = Some("Device is not connected.".to_owned());
                    }
                }

                KeyCode::Char('u') => {
                    if self.mqtt_ctrl.is_device_connected() {
                        self.config_result = Some(ModuleInfo::undeployment_json());
                    } else {
                        self.app_error = Some("Device is not connected.".to_owned());
                    }
                }

                KeyCode::Char('s') => {
                    if self.mqtt_ctrl.is_device_connected() {
                        if let Some(Ok(deploy)) = &self.config_result {
                            match self.mqtt_ctrl.send_configure(deploy) {
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
                    if let Some(azurite_storage) = &mut self.azurite_storage {
                        azurite_storage.current_module_focus_up();
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if let Some(azurite_storage) = &mut self.azurite_storage {
                        azurite_storage.current_module_focus_down();
                    }
                }
                _ => {}
            },
            DMScreen::EdgeApp(state) => match state {
                DMScreenState::DefaultState => match key_event.code {
                    KeyCode::Esc => self.dm_screen_move_back(),
                    KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                    KeyCode::Char('e') => {
                        self.config_key_focus_start = ConfigKey::CommonSettingsProcessState.into();
                        self.config_key_focus_end = ConfigKey::CommonSettingsUploadInterval.into();
                        self.config_key_focus = self.config_key_focus_start;
                        self.dm_screen_move_to(DMScreen::EdgeApp(DMScreenState::ConfigureState));
                    }
                    _ => {}
                },
                DMScreenState::ConfigureState => match key_event.code {
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
                        if let Some(edge_app) = self.mqtt_ctrl().edge_app() {
                            self.config_result = match edge_app.parse_configure(&self.config_keys) {
                                Ok(s) => Some(Ok(s)),
                                Err(e) => Some(Err(e)),
                            };
                            self.dm_screen_move_to(DMScreen::EdgeApp(DMScreenState::ResultState));
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
                DMScreenState::ResultState => match key_event.code {
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
        }
    }

    pub fn should_exit(&self) -> bool {
        self.exit
    }

    pub fn mqtt_ctrl(&self) -> &MqttCtrl {
        &self.mqtt_ctrl
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

        if self.current_screen() == DMScreen::Main {
            if let Err(e) = ui_main::draw(chunks[1], buf, self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }

            jinfo!(
                event = "TIME_MEASURE",
                draw_main_time = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }

        if self.current_screen() == DMScreen::Module {
            if let Err(e) = ui_module::draw(chunks[1], buf, self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }

            jinfo!(
                event = "TIME_MEASURE",
                draw_module_time = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }

        if self.current_screen() == DMScreen::Configuration {
            if let Err(e) = ui_config::draw(chunks[1], buf, self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }
        }

        if self.current_screen() == DMScreen::ConfigurationUser {
            if let Err(e) = ui_config_user::draw(chunks[1], buf, self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }
        }

        if self.current_screen() == DMScreen::DirectCommand {
            if let Err(e) = ui_directcmd::draw(chunks[1], buf, self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }
        }

        if self.current_screen() == DMScreen::EvpModule {
            if let Err(e) = ui_evp_module::draw(chunks[1], buf, self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }
        }

        if self.current_screen() == DMScreen::Elog {
            if let Err(e) = ui_elog::draw(chunks[1], buf, self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }
        }

        if self.current_screen() == DMScreen::EdgeApp(DMScreenState::DefaultState)
            || self.current_screen() == DMScreen::EdgeApp(DMScreenState::ConfigureState)
            || self.current_screen() == DMScreen::EdgeApp(DMScreenState::ResultState)
        {
            if let Err(e) = ui_edge_app::draw(chunks[1], buf, self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }
        }

        if self.current_screen() == DMScreen::Exiting {
            if let Err(e) = ui_exit::draw(chunks[1], buf, self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }
            jinfo!(
                event = "TIME_MEASURE",
                draw_exit_time = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }

        if let Err(e) = ui_foot::draw(chunks[2], buf, self) {
            jerror!(func = "App::render()", error = format!("{:?}", e));
        }
    }
}
