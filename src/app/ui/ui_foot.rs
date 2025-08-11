use crate::mqtt_ctrl::with_mqtt_ctrl;
#[allow(unused)]
use {
    crate::{
        app::{App, DMScreen, DMScreenState, DirectCommand, MainWindowFocus},
        azurite::{AzuriteAction, AzuriteStorage, with_azurite_storage},
        error::DMError,
        mqtt_ctrl::{
            MqttCtrl,
            evp::device_info::{ChipInfo, DeviceInfo},
            evp::evp_state::{AgentDeviceConfig, AgentSystemInfo, UUID},
            evp::{
                device_info::{
                    DeviceCapabilities, DeviceReserved, DeviceStates, NetworkSettings,
                    SystemSettings, WirelessSettings,
                },
                evp_state::DeploymentStatus,
            },
        },
    },
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
};

pub fn draw(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    with_mqtt_ctrl(|mqtt_ctrl| -> Result<(), DMError> {
        // Draw foot
        let foot_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
            .split(area);

        // Draw the current connection status and last connected time
        let mut connect_info = Span::styled(" Disconnected ", Style::default().fg(Color::Red));

        let is_device_connected = mqtt_ctrl.is_device_connected();
        let last_connected = mqtt_ctrl.last_connected_time();
        let now = Local::now();
        let delta = now - last_connected;
        let days = delta.num_days();
        let hours = delta.num_hours() % 24;
        let minutes = delta.num_minutes() % 60;
        let seconds = delta.num_seconds() % 60;

        let last_connected_str = format!(
            "{} ({} day {}h {}m {}s ago)",
            last_connected.format("%Y-%m-%d %H:%M:%S"),
            days,
            hours,
            minutes,
            seconds
        );
        let mut last_connected_info =
            Span::styled(&last_connected_str, Style::default().fg(Color::DarkGray));

        if is_device_connected {
            connect_info = Span::styled(" Connected ", Style::default().fg(Color::Green));
            last_connected_info =
                Span::styled(&last_connected_str, Style::default().fg(Color::White));
        }

        let current_navigation_text = vec![
            connect_info,
            Span::styled(" | ", Style::default().fg(Color::White)),
            last_connected_info,
        ];
        Paragraph::new(Line::from(current_navigation_text))
            .block(Block::default().borders(Borders::NONE))
            .render(foot_chunks[0], buf);

        if let Some(error) = app.app_error.as_ref() {
            // If there is an error, display it in red
            Paragraph::new(Line::from(Span::styled(
                error,
                Style::default().fg(Color::Red),
            )))
            .render(foot_chunks[1], buf);
        } else if let Some(info) = mqtt_ctrl.info.as_ref() {
            // If there is info, display it in white
            Paragraph::new(Line::from(Span::styled(
                info,
                Style::default().fg(Color::White),
            )))
            .render(foot_chunks[1], buf);
        } else {
            // Shows current keys hint based on the screen and focus
            let current_keys_hint = match app.current_screen() {
                DMScreen::Main => match app.main_window_focus() {
                    MainWindowFocus::AgentState
                    | MainWindowFocus::SystemSettings
                    | MainWindowFocus::NetworkSettings
                    | MainWindowFocus::WirelessSettings => Span::styled(
                        "UP(k)/DOWN(j)/LEFT(h)/RIGHT(l) move, (ENTER) detail, (e)/(E) edit, (d) DirectCmd, (m) ModuleOp, (t) TokenProvider, (g) elog, (q) quit",
                        Style::default().fg(Color::White),
                    ),
                    MainWindowFocus::DeviceState
                    | MainWindowFocus::MainChip
                    | MainWindowFocus::SensorChip
                    | MainWindowFocus::CompanionChip
                    | MainWindowFocus::DeviceManifest
                    | MainWindowFocus::DeviceReserved
                    | MainWindowFocus::DeploymentStatus
                    | MainWindowFocus::DeviceCapabilities => Span::styled(
                        "UP(k)/DOWN(j) move, (Enter) detail, (d) DirectCmd, (m) ModuleOp, (t) TokenProvider, (g) elog, (q) quit",
                        Style::default().fg(Color::White),
                    ),
                },
                DMScreen::Module => match app.main_window_focus() {
                    MainWindowFocus::MainChip
                    | MainWindowFocus::SensorChip
                    | MainWindowFocus::CompanionChip
                    | MainWindowFocus::AgentState
                    | MainWindowFocus::SystemSettings
                    | MainWindowFocus::NetworkSettings
                    | MainWindowFocus::DeploymentStatus
                    | MainWindowFocus::WirelessSettings => Span::styled(
                        "(e)/(E) edit (d) DirectCmd, (m) ModuleOp, (g) elog, (ENTER)/(ESC) back, (q) quit",
                        Style::default().fg(Color::White),
                    ),
                    MainWindowFocus::DeviceState
                    | MainWindowFocus::DeviceManifest
                    | MainWindowFocus::DeviceReserved
                    | MainWindowFocus::DeviceCapabilities => Span::styled(
                        "(d) DirectCmd, (m) ModuleOp, (g) elog, (ENTER)/(ESC) back, (q) quit",
                        Style::default().fg(Color::White),
                    ),
                },

                DMScreen::Elog => Span::styled(
                    "(w) save, (ESC) back, (q) quit",
                    Style::default().fg(Color::White),
                ),

                DMScreen::Configuration => {
                    if app.config_result.is_none() {
                        Span::styled(
                            "(ESC):back, UP(k)/DOWN(j) move, (a)/(i) edit, (w) write",
                            Style::default().fg(Color::White),
                        )
                    } else {
                        Span::styled("(ESC) back, (s) send", Style::default().fg(Color::White))
                    }
                }

                DMScreen::ConfigurationUser => {
                    if app.config_result.is_none() {
                        Span::styled(
                            "(q) quit, (ESC) back, (w) write",
                            Style::default().fg(Color::White),
                        )
                    } else {
                        Span::styled(
                            "(q) quit, (ESC) back, (s) send",
                            Style::default().fg(Color::White),
                        )
                    }
                }

                DMScreen::DirectCommand => {
                    if let Some(DirectCommand::GetDirectImage) = mqtt_ctrl.get_direct_command() {
                        if mqtt_ctrl.direct_command_request().is_none() {
                            Span::styled(
                                "(ESC) back, UP(k)/DOWN(j) move, (a)/(i) edit, (s) send",
                                Style::default().fg(Color::White),
                            )
                        } else if let Some(Ok(_)) = mqtt_ctrl.direct_command_result() {
                            Span::styled(
                                "(ESC) back, (w) save (q) quit",
                                Style::default().fg(Color::White),
                            )
                        } else {
                            Span::styled(
                                "(ESC) back, (s) send (q) quit",
                                Style::default().fg(Color::White),
                            )
                        }
                    } else {
                        Span::styled("(ESC) back, (q) quit", Style::default().fg(Color::White))
                    }
                }

                DMScreen::EvpModule => {
                    if let Some(action) =
                        with_azurite_storage(|azure_storage| azure_storage.action())
                    {
                        if action == AzuriteAction::Add {
                            Span::styled(
                                "(ESC) back, (ENTER) register",
                                Style::default().fg(Color::White),
                            )
                        } else if app.config_result.is_some() {
                            Span::styled(
                                "(s) send, (ESC) back, (q) quit",
                                Style::default().fg(Color::White),
                            )
                        } else {
                            Span::styled(
                                "UP(k)/DOWN(j) move, (a) add, (r) remove, (d) deploy, (u) undeploy, (ESC) back, (q) quit",
                                Style::default().fg(Color::White),
                            )
                        }
                    } else {
                        Span::styled("", Style::default().fg(Color::White))
                    }
                }
                DMScreen::TokenProvider => {
                    if app.token_provider_for_config.is_some() {
                        Span::styled(
                            "UP(k)/DOWN(j) move, (ENTER) select, (a) add, (d) delete, (ESC) back, (q) quit",
                            Style::default().fg(Color::White),
                        )
                    } else {
                        Span::styled(
                            "UP(k)/DOWN(j) move, (a) add, (d) delete, (s) set current, (ESC) back, (q) quit",
                            Style::default().fg(Color::White),
                        )
                    }
                }

                DMScreen::EdgeApp(state) => match state {
                    DMScreenState::Initial => Span::styled(
                        "(e) edit, (ESC) back, (q) quit",
                        Style::default().fg(Color::White),
                    ),
                    DMScreenState::Configuring => Span::styled(
                        "UP(k)/DOWN(j) move, (w) write, (ESC) back, (q) quit",
                        Style::default().fg(Color::White),
                    ),
                    DMScreenState::Completed => Span::styled(
                        "(s) send, (ESC) back, (q) quit",
                        Style::default().fg(Color::White),
                    ),
                },

                DMScreen::Exiting => {
                    Span::styled("(y) exit / (n) cancel", Style::default().fg(Color::White))
                }
            };

            Paragraph::new(Line::from(format!(" {}", current_keys_hint)))
                .block(Block::default().borders(Borders::LEFT))
                .render(foot_chunks[1], buf);
        }

        Ok(())
    })
}
