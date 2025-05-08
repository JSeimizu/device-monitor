mod ui;

#[allow(unused)]
use {
    super::{error::DMError, mqtt_ctrl::MqttCtrl},
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
    ui::{ui_exit, ui_main},
};

pub struct AppConfig {
    pub broker: Option<String>,
}

#[derive(Debug, Default, PartialEq)]
pub enum CurrentScreen {
    #[default]
    Main,
    Editing,
    Exiting,
}

#[derive(Debug, Default, PartialEq, PartialOrd)]
pub enum CurrentlyEditing {
    #[default]
    None,
    Key,
    Value,
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
    key_input: Option<String>,
    value_input: Option<String>,
    pairs: HashMap<String, String>,
    current_screen: CurrentScreen,
    main_window_focus: MainWindowFocus,
    currently_editing: CurrentlyEditing,
}

impl App {
    pub fn new(cfg: AppConfig) -> Result<Self, DMError> {
        let broker = cfg.broker.as_deref().unwrap_or("localhost:1883");
        let (broker_url, broker_port_str) = broker.split_once(':').unwrap();
        let broker_port = broker_port_str.parse().unwrap_or(1883);

        let mqtt_ctrl = MqttCtrl::new(broker_url, broker_port)?;

        Ok(Self {
            mqtt_ctrl,
            exit: false,
            should_print_json: false,
            key_input: None,
            value_input: None,
            pairs: HashMap::new(),
            current_screen: CurrentScreen::Main,
            main_window_focus: MainWindowFocus::default(),
            currently_editing: CurrentlyEditing::None,
        })
    }

    pub fn save_key_value(&mut self) {
        self.pairs.insert(
            self.key_input.take().unwrap_or_default(),
            self.value_input.take().unwrap_or_default(),
        );
        self.currently_editing = CurrentlyEditing::None;
    }

    pub fn toggle_editing(&mut self) {
        let next = match self.currently_editing {
            CurrentlyEditing::Key => CurrentlyEditing::Value,
            CurrentlyEditing::Value => CurrentlyEditing::Key,
            CurrentlyEditing::None => CurrentlyEditing::None,
        };
        self.currently_editing = next;
    }

    pub fn print_json(&self) -> Result<(), DMError> {
        if self.should_print_json {
            let output = serde_json::to_string(&self.pairs)
                .map_err(|e| Report::new(DMError::InvalidData).attach_printable(e))?;
            println!("{}", output);
        }
        Ok(())
    }

    pub fn update(&mut self) -> Result<(), DMError> {
        self.pairs.extend(self.mqtt_ctrl.update()?);
        Ok(())
    }

    pub fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    pub fn handle_events(&mut self) -> Result<(), DMError> {
        let has_new_event = event::poll(Duration::from_millis(500))
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

    pub fn handle_key_event(&mut self, key_event: KeyEvent) {
        match self.current_screen {
            CurrentScreen::Main => match key_event.code {
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
                KeyCode::Char('e') => {
                    self.current_screen = CurrentScreen::Editing;
                    self.currently_editing = CurrentlyEditing::Key;
                }
                KeyCode::Char('q') => {
                    self.current_screen = CurrentScreen::Exiting;
                }
                _ => {}
            },
            CurrentScreen::Exiting => {
                match key_event.code {
                    KeyCode::Char('y') => {
                        self.exit = true;
                    }
                    KeyCode::Char('n') => {
                        self.current_screen = CurrentScreen::Main;
                        self.exit = false;
                    }
                    _ => {}
                };
            }
            CurrentScreen::Editing => match key_event.code {
                KeyCode::Enter => match self.currently_editing {
                    CurrentlyEditing::Key => {
                        self.currently_editing = CurrentlyEditing::Value;
                    }
                    CurrentlyEditing::Value => {
                        self.save_key_value();
                        self.current_screen = CurrentScreen::Main;
                    }
                    _ => {}
                },
                KeyCode::Backspace => match self.currently_editing {
                    CurrentlyEditing::Key => {
                        if let Some(input) = &mut self.key_input {
                            input.pop();
                        }
                    }
                    CurrentlyEditing::Value => {
                        if let Some(input) = &mut self.value_input {
                            input.pop();
                        }
                    }
                    _ => {}
                },
                KeyCode::Esc => {
                    self.current_screen = CurrentScreen::Main;
                    self.currently_editing = CurrentlyEditing::None;
                }
                KeyCode::Tab => {
                    self.toggle_editing();
                }
                KeyCode::Char(value) => match self.currently_editing {
                    CurrentlyEditing::Key => {
                        if let Some(input) = &mut self.key_input {
                            input.push(value);
                        } else {
                            self.key_input = Some(value.to_string());
                        }
                    }
                    CurrentlyEditing::Value => {
                        if let Some(input) = &mut self.value_input {
                            input.push(value);
                        } else {
                            self.value_input = Some(value.to_string());
                        }
                    }
                    _ => {}
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
        if self.current_screen == CurrentScreen::Main {
            if let Err(e) = ui_main::draw(area, buf, &self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }

            jdebug!(
                func = "App::render()",
                draw_main_time = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }

        if self.current_screen == CurrentScreen::Exiting {
            if let Err(e) = ui_exit::draw(area, buf, &self) {
                jerror!(func = "App::render()", error = format!("{:?}", e));
            }
            jdebug!(
                func = "App::render()",
                draw_exit_time = format!("{}ms", draw_start.elapsed().as_millis())
            )
        }
    }
}
