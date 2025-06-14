use crate::app::MainWindowFocus;
#[allow(unused)]
use {
    super::*,
    crate::{
        app::{App, DMScreen},
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
    match app.main_window_focus {
        MainWindowFocus::MainChip => {
            let device_info = app.mqtt_ctrl().device_info();
            draw_chip_info(area, buf, device_info, "main_chip", BlockType::Normal)
        }

        MainWindowFocus::CompanionChip => {
            let device_info = app.mqtt_ctrl().device_info();
            draw_chip_info(area, buf, device_info, "companion_chip", BlockType::Normal)
        }

        MainWindowFocus::SensorChip => {
            let device_info = app.mqtt_ctrl().device_info();
            draw_chip_info(area, buf, device_info, "sensor_chip", BlockType::Normal)
        }

        MainWindowFocus::DeviceManifest => {
            draw_device_manifest(area, buf, app.mqtt_ctrl().device_info(), BlockType::Normal)
        }

        MainWindowFocus::AgentState => {
            let agent_system_info = app.mqtt_ctrl().agent_system_info();
            let agent_device_config = app.mqtt_ctrl().agent_device_config();
            draw_agent_state(
                area,
                buf,
                agent_system_info,
                agent_device_config,
                BlockType::Normal,
            )
        }

        MainWindowFocus::DeploymentStatus => {
            let deployment_status = app.mqtt_ctrl.deployment_status();
            draw_deployment_status(area, buf, deployment_status, BlockType::Normal)
        }

        MainWindowFocus::DeviceReserved => {
            let device_reserved = app.mqtt_ctrl().device_reserved();
            draw_device_reserved(area, buf, device_reserved, BlockType::Normal)
        }

        MainWindowFocus::DeviceState => {
            let device_states = app.mqtt_ctrl().device_states();
            draw_device_states(area, buf, device_states, BlockType::Normal)
        }

        MainWindowFocus::DeviceCapabilities => {
            let device_capabilities = app.mqtt_ctrl().device_capabilities();
            draw_device_capabilities(area, buf, device_capabilities, BlockType::Normal)
        }

        MainWindowFocus::SystemSettings => {
            let system_settings = app.mqtt_ctrl().system_settings();
            draw_system_settings(area, buf, system_settings, BlockType::Normal)
        }

        MainWindowFocus::NetworkSettings => {
            let network_settings = app.mqtt_ctrl().network_settings();
            draw_network_settings(area, buf, network_settings, BlockType::Normal)
        }

        MainWindowFocus::WirelessSettings => {
            let wireless_settings = app.mqtt_ctrl().wireless_settings();
            draw_wireless_settings(area, buf, wireless_settings, BlockType::Normal)
        }
    }
}
