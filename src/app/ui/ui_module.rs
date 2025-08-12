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

use crate::{app::MainWindowFocus, mqtt_ctrl::with_mqtt_ctrl};
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
    with_mqtt_ctrl(|mqtt_ctrl| -> Result<(), DMError> {
        match app.main_window_focus {
            MainWindowFocus::MainChip => {
                let device_info = mqtt_ctrl.device_info();
                draw_chip_info(area, buf, device_info, "main_chip", BlockType::Normal)
            }

            MainWindowFocus::CompanionChip => {
                let device_info = mqtt_ctrl.device_info();
                draw_chip_info(area, buf, device_info, "companion_chip", BlockType::Normal)
            }

            MainWindowFocus::SensorChip => {
                let device_info = mqtt_ctrl.device_info();
                draw_chip_info(area, buf, device_info, "sensor_chip", BlockType::Normal)
            }

            MainWindowFocus::DeviceManifest => {
                draw_device_manifest(area, buf, mqtt_ctrl.device_info(), BlockType::Normal)
            }

            MainWindowFocus::AgentState => {
                let agent_system_info = mqtt_ctrl.agent_system_info();
                let agent_device_config = mqtt_ctrl.agent_device_config();
                draw_agent_state(
                    area,
                    buf,
                    agent_system_info,
                    agent_device_config,
                    BlockType::Normal,
                )
            }

            MainWindowFocus::DeploymentStatus => {
                let deployment_status = mqtt_ctrl.deployment_status();
                draw_deployment_status(area, buf, deployment_status, BlockType::Normal)
            }

            MainWindowFocus::DeviceReserved => {
                let device_reserved = mqtt_ctrl.device_reserved();
                draw_device_reserved(area, buf, device_reserved, BlockType::Normal)
            }

            MainWindowFocus::DeviceState => {
                let device_states = mqtt_ctrl.device_states();
                draw_device_states(area, buf, device_states, BlockType::Normal)
            }

            MainWindowFocus::DeviceCapabilities => {
                let device_capabilities = mqtt_ctrl.device_capabilities();
                draw_device_capabilities(area, buf, device_capabilities, BlockType::Normal)
            }

            MainWindowFocus::SystemSettings => {
                let system_settings = mqtt_ctrl.system_settings();
                draw_system_settings(area, buf, system_settings, BlockType::Normal)
            }

            MainWindowFocus::NetworkSettings => {
                let network_settings = mqtt_ctrl.network_settings();
                draw_network_settings(area, buf, network_settings, BlockType::Normal)
            }

            MainWindowFocus::WirelessSettings => {
                let wireless_settings = mqtt_ctrl.wireless_settings();
                draw_wireless_settings(area, buf, wireless_settings, BlockType::Normal)
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    #[should_panic]
    fn test_draw_panics_when_mqtt_uninitialized() {
        // Construct an App via the public constructor
        let app = crate::app::App::new(crate::app::AppConfig { broker: "b" }).unwrap();

        // Prepare drawing area and buffer
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);

        // draw() uses with_mqtt_ctrl which will panic when the global MqttCtrl is not initialized.
        let _ = draw(area, &mut buf, &app);
    }
}
