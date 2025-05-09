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
    MainChip,
    CompanionChip,
    DeviceManifest,
    SensorChip,
    SystemSettings,
    NetworkSettings,
    WirelessSettings,
    DeploymentStatus,
    DeviceState,
    DeviceCapabilities,
    DeviceReserved,
    AgentState,
    Configuration,
    Exiting,
}

#[derive(Debug, Default, PartialEq, PartialOrd, Clone, Copy)]
pub enum ConfigKey {
    #[default]
    ReportStatusIntervalMin,
    ReportStatusIntervalMax,
    Invalid,
}

impl From<usize> for ConfigKey {
    fn from(value: usize) -> Self {
        match value {
            0 => ConfigKey::ReportStatusIntervalMin,
            1 => ConfigKey::ReportStatusIntervalMax,
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

pub struct App {
    exit: bool,
    should_print_json: bool,
    mqtt_ctrl: MqttCtrl,
    pairs: HashMap<String, String>,
    screens: Vec<DMScreen>,
    main_window_focus: MainWindowFocus,
    config_keys: Vec<String>,
    config_key_focus: usize,
    config_key_editable: bool,
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
            config_key_editable: false,
        })
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
    }

    pub fn dm_screen_move_back(&mut self) {
        if self.screens.len() > 1 {
            self.screens.pop();
        }
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
        if self.config_key_focus == 0 {
            self.config_key_focus = self.config_keys.len() - 1;
        } else {
            self.config_key_focus -= 1;
        }
    }

    pub fn config_focus_down(&mut self) {
        self.config_key_focus += 1;
        if self.config_key_focus == self.config_keys.len() {
            self.config_key_focus = 0;
        }
    }

    pub fn config_key_clear(&mut self) {
        self.config_keys = (0..ConfigKey::size() - 1).map(|_| String::new()).collect();
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
                KeyCode::Enter => match self.main_window_focus {
                    MainWindowFocus::CompanionChip => {
                        self.dm_screen_move_to(DMScreen::CompanionChip)
                    }
                    MainWindowFocus::SystemSettings => {
                        self.dm_screen_move_to(DMScreen::SystemSettings)
                    }
                    MainWindowFocus::NetworkSettings => {
                        self.dm_screen_move_to(DMScreen::NetworkSettings)
                    }
                    MainWindowFocus::WirelessSettings => {
                        self.dm_screen_move_to(DMScreen::WirelessSettings)
                    }
                    MainWindowFocus::DeploymentStatus => {
                        self.dm_screen_move_to(DMScreen::DeploymentStatus)
                    }
                    MainWindowFocus::DeviceState => self.dm_screen_move_to(DMScreen::DeviceState),
                    MainWindowFocus::DeviceCapabilities => {
                        self.dm_screen_move_to(DMScreen::DeviceCapabilities);
                    }
                    MainWindowFocus::DeviceReserved => {
                        self.dm_screen_move_to(DMScreen::DeviceReserved);
                    }
                    MainWindowFocus::AgentState => self.dm_screen_move_to(DMScreen::AgentState),
                    MainWindowFocus::MainChip => self.dm_screen_move_to(DMScreen::MainChip),
                    MainWindowFocus::SensorChip => self.dm_screen_move_to(DMScreen::SensorChip),
                    MainWindowFocus::DeviceManifest => {
                        self.dm_screen_move_to(DMScreen::DeviceManifest);
                    }
                },
                KeyCode::Char('e') => {
                    self.config_key_clear();
                    self.dm_screen_move_to(DMScreen::Configuration);
                }
                KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                _ => {}
            },
            DMScreen::CompanionChip
            | DMScreen::DeviceManifest
            | DMScreen::SensorChip
            | DMScreen::MainChip
            | DMScreen::AgentState
            | DMScreen::DeviceReserved
            | DMScreen::DeviceCapabilities
            | DMScreen::DeviceState
            | DMScreen::SystemSettings
            | DMScreen::NetworkSettings
            | DMScreen::WirelessSettings
            | DMScreen::DeploymentStatus => match key_event.code {
                KeyCode::Enter | KeyCode::Esc => self.dm_screen_move_back(),
                KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
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
                KeyCode::Enter => {}
                KeyCode::Esc => self.dm_screen_move_back(),
                KeyCode::Up | KeyCode::Char('k') => self.config_focus_up(),
                KeyCode::Down | KeyCode::Char('j') => self.config_focus_down(),
                KeyCode::Char('q') => self.dm_screen_move_to(DMScreen::Exiting),
                KeyCode::Tab => self.config_focus_down(),
                KeyCode::Char('i') | KeyCode::Char('a') => self.config_key_editable = true,
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
        if self.current_screen() == DMScreen::CompanionChip {
            if let Err(e) = ui_companion_chip::draw(chunks[1], buf, &self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }

            jinfo!(
                event = "TIME_MEASURE",
                draw_companion_chip = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }

        if self.current_screen() == DMScreen::SystemSettings {
            if let Err(e) = ui_system_settings::draw(chunks[1], buf, &self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }

            jinfo!(
                event = "TIME_MEASURE",
                draw_system_settings = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }

        if self.current_screen() == DMScreen::NetworkSettings {
            if let Err(e) = ui_network_settings::draw(chunks[1], buf, &self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }

            jinfo!(
                event = "TIME_MEASURE",
                draw_network_settings = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }

        if self.current_screen() == DMScreen::WirelessSettings {
            if let Err(e) = ui_wireless_settings::draw(chunks[1], buf, &self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }

            jinfo!(
                event = "TIME_MEASURE",
                draw_wireless_settings = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }

        if self.current_screen() == DMScreen::DeploymentStatus {
            if let Err(e) = ui_deployment_status::draw(chunks[1], buf, &self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }

            jinfo!(
                event = "TIME_MEASURE",
                draw_deployment_status = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }

        if self.current_screen() == DMScreen::DeviceState {
            if let Err(e) = ui_device_state::draw(chunks[1], buf, &self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }

            jinfo!(
                event = "TIME_MEASURE",
                draw_device_states = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }

        if self.current_screen() == DMScreen::DeviceCapabilities {
            if let Err(e) = ui_device_capabilities::draw(chunks[1], buf, &self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }

            jinfo!(
                event = "TIME_MEASURE",
                draw_device_capabilities = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }

        if self.current_screen() == DMScreen::DeviceReserved {
            if let Err(e) = ui_device_reserved::draw(chunks[1], buf, &self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }

            jinfo!(
                event = "TIME_MEASURE",
                draw_device_reserved = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }
        if self.current_screen() == DMScreen::DeviceCapabilities {
            if let Err(e) = ui_device_capabilities::draw(chunks[1], buf, &self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }

            jinfo!(
                event = "TIME_MEASURE",
                draw_device_capabilities = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }

        if self.current_screen() == DMScreen::AgentState {
            if let Err(e) = ui_agent_state::draw(chunks[1], buf, &self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }

            jinfo!(
                event = "TIME_MEASURE",
                draw_agent_state = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }

        if self.current_screen() == DMScreen::MainChip {
            if let Err(e) = ui_main_chip::draw(chunks[1], buf, &self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }

            jinfo!(
                event = "TIME_MEASURE",
                draw_main_chip = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }

        if self.current_screen() == DMScreen::SensorChip {
            if let Err(e) = ui_sensor_chip::draw(chunks[1], buf, &self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }

            jinfo!(
                event = "TIME_MEASURE",
                draw_sensor_chip = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }

        if self.current_screen() == DMScreen::DeviceManifest {
            if let Err(e) = ui_device_manifest::draw(chunks[1], buf, &self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }

            jinfo!(
                event = "TIME_MEASURE",
                draw_device_manifest = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }

        if self.current_screen() == DMScreen::Configuration {
            if let Err(e) = ui_config::draw(chunks[1], buf, &self) {
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
