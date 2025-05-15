mod ui;

#[allow(unused)]
use {
    super::{error::DMError, mqtt_ctrl::MqttCtrl},
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
        io,
        time::{Duration, Instant},
    },
    ui::*,
};

pub struct AppConfig<'a> {
    pub broker: &'a str,
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub enum DMScreen {
    #[default]
    Main,
    Module,
    Configuration,
    ConfigurationUser,
    Exiting,
}

#[derive(Debug, Default, PartialEq, PartialOrd, Clone, Copy)]
pub enum ConfigKey {
    #[default]
    //AgentState
    ReportStatusIntervalMin,
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
    ProxyUrl,
    StaticIpv4Ip,
    StaticIpv4SubnetMask,
    StaticIpv4Gateway,
    StaticIpv4Dns,
    StaticIpv6Ip,
    StaticIpv6SubnetMask,
    StaticIpv6Gateway,
    StaticIpv6Dns,
    ProxyPort,
    ProxyUserName,
    ProxyPassword,

    // Wireless settings
    StaSsid,
    StaPassword,
    StaEncryption,

    Invalid,
}

impl From<usize> for ConfigKey {
    fn from(value: usize) -> Self {
        match value {
            0 => ConfigKey::ReportStatusIntervalMin,
            1 => ConfigKey::ReportStatusIntervalMax,
            2 => ConfigKey::LedEnabled,
            3 => ConfigKey::TemperatureUpdateInterval,
            4 => ConfigKey::AllLogSettingLevel,
            5 => ConfigKey::AllLogSettingDestination,
            6 => ConfigKey::AllLogSettingStorageName,
            7 => ConfigKey::AllLogSettingPath,
            8 => ConfigKey::MainLogSettingLevel,
            9 => ConfigKey::MainLogSettingDestination,
            10 => ConfigKey::MainLogSettingStorageName,
            11 => ConfigKey::MainLogSettingPath,
            12 => ConfigKey::SensorLogSettingLevel,
            13 => ConfigKey::SensorLogSettingDestination,
            14 => ConfigKey::SensorLogSettingStorageName,
            15 => ConfigKey::SensorLogSettingPath,
            16 => ConfigKey::CompanionFwLogSettingLevel,
            17 => ConfigKey::CompanionFwLogSettingDestination,
            18 => ConfigKey::CompanionFwLogSettingStorageName,
            19 => ConfigKey::CompanionFwLogSettingPath,
            20 => ConfigKey::CompanionAppLogSettingLevel,
            21 => ConfigKey::CompanionAppLogSettingDestination,
            22 => ConfigKey::CompanionAppLogSettingStorageName,
            23 => ConfigKey::CompanionAppLogSettingPath,
            24 => ConfigKey::IpMethod,
            25 => ConfigKey::NtpUrl,
            26 => ConfigKey::StaticIpv4Ip,
            27 => ConfigKey::StaticIpv4SubnetMask,
            28 => ConfigKey::StaticIpv4Gateway,
            29 => ConfigKey::StaticIpv4Dns,
            30 => ConfigKey::StaticIpv6Ip,
            31 => ConfigKey::StaticIpv6SubnetMask,
            32 => ConfigKey::StaticIpv6Gateway,
            33 => ConfigKey::StaticIpv6Dns,
            34 => ConfigKey::ProxyUrl,
            35 => ConfigKey::ProxyPort,
            36 => ConfigKey::ProxyUserName,
            37 => ConfigKey::ProxyPassword,
            38 => ConfigKey::StaSsid,
            39 => ConfigKey::StaPassword,
            40 => ConfigKey::StaEncryption,
            _ => ConfigKey::Invalid,
        }
    }
}

impl From<ConfigKey> for usize {
    fn from(value: ConfigKey) -> Self {
        for i in 0..ConfigKey::size() {
            // Max value is the number for ConfigKey::Invalid
            if ConfigKey::from(i) == value {
                return i;
            }
        }

        // impossible to come here
        ConfigKey::size()
    }
}

impl ConfigKey {
    pub fn size() -> usize {
        let mut result = 0;
        for i in 0..usize::MAX {
            if ConfigKey::from(i) == ConfigKey::Invalid {
                result = i + 1;
                break;
            }
        }

        result
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
            _ => "configure.json",
        }
    }
}

pub struct App {
    exit: bool,
    should_print_json: bool,
    mqtt_ctrl: MqttCtrl,
    pairs: HashMap<String, String>,
    screens: Vec<DMScreen>,
    main_window_focus: MainWindowFocus,
    config_keys: Vec<String>,
    config_key_focus: usize,
    config_key_focus_start: usize,
    config_key_focus_end: usize,
    config_key_editable: bool,
    config_result: Option<Result<String, DMError>>,
    app_error: Option<String>,
}

impl App {
    pub fn new(cfg: AppConfig) -> Result<Self, DMError> {
        let broker = cfg.broker;
        let (broker_url, broker_port_str) = broker.split_once(':').unwrap();
        let broker_port = broker_port_str.parse().unwrap_or(1883);

        let mqtt_ctrl = MqttCtrl::new(broker_url, broker_port)?;

        Ok(Self {
            mqtt_ctrl,
            exit: false,
            should_print_json: false,
            pairs: HashMap::new(),
            screens: vec![DMScreen::Main],
            main_window_focus: MainWindowFocus::default(),
            config_keys: (0..ConfigKey::size() - 1).map(|_| String::new()).collect(),
            config_key_focus: 0,
            config_key_focus_start: 0,
            config_key_focus_end: 0,
            config_key_editable: false,
            config_result: None,
            app_error: None,
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

    pub fn print_json(&self) -> Result<(), DMError> {
        if self.should_print_json {
            let output = serde_json::to_string(&self.pairs)
                .map_err(|e| Report::new(DMError::InvalidData).attach_printable(e))?;
            println!("{}", output);
        }
        Ok(())
    }

    pub fn current_screen(&self) -> DMScreen {
        self.screens.last().unwrap().to_owned()
    }

    pub fn dm_screen_move_to(&mut self, next_screen: DMScreen) {
        self.screens.push(next_screen);
        self.app_error = None;
    }

    pub fn dm_screen_move_back(&mut self) {
        if self.screens.len() > 1 {
            self.screens.pop();
        }

        self.app_error = None;
    }

    pub fn update(&mut self) -> Result<(), DMError> {
        self.pairs.extend(self.mqtt_ctrl.update()?);
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

    pub fn switch_to_config_screen(&mut self, user_config: bool) {
        if self.mqtt_ctrl.is_device_connected() {
            self.config_key_clear();
            if user_config {
                self.dm_screen_move_to(DMScreen::ConfigurationUser);
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
                _ => {}
            },

            DMScreen::Module => match key_event.code {
                KeyCode::Enter | KeyCode::Esc => self.dm_screen_move_back(),
                KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                KeyCode::Char('e') => self.switch_to_config_screen(false),
                KeyCode::Char('E') => self.switch_to_config_screen(true),
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
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
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

        if let Err(e) = ui_head::draw(chunks[0], buf, &self) {
            jerror!(func = "App::render()", error = format!("{:?}", e));
        }

        if self.current_screen() == DMScreen::Main {
            if let Err(e) = ui_main::draw(chunks[1], buf, &self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }

            jinfo!(
                event = "TIME_MEASURE",
                draw_main_time = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }

        if self.current_screen() == DMScreen::Module {
            if let Err(e) = ui_module::draw(chunks[1], buf, &self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }

            jinfo!(
                event = "TIME_MEASURE",
                draw_module_time = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }

        if self.current_screen() == DMScreen::Configuration {
            if let Err(e) = ui_config::draw(chunks[1], buf, &self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }
        }

        if self.current_screen() == DMScreen::ConfigurationUser {
            if let Err(e) = ui_config_user::draw(chunks[1], buf, &self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }
        }

        if self.current_screen() == DMScreen::Exiting {
            if let Err(e) = ui_exit::draw(chunks[1], buf, &self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }
            jinfo!(
                event = "TIME_MEASURE",
                draw_exit_time = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }

        if let Err(e) = ui_foot::draw(chunks[2], buf, &self) {
            jerror!(func = "App::render()", error = format!("{:?}", e));
        }
    }
}
