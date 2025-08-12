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

use crate::app::{App, MainWindowFocus};

#[allow(unused)]
use {
    super::*,
    crate::{
        app::DMScreen,
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
            with_mqtt_ctrl,
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
    // Draw body
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(area);

    let body_sub_chunks_left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(10),
        ])
        .split(body_chunks[0]);

    let body_sub_chunks_middle = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
            Constraint::Percentage(15),
        ])
        .split(body_chunks[1]);

    let body_sub_chunks_right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .split(body_chunks[2]);

    let get_block_type = |focus| {
        if focus == app.main_window_focus() {
            BlockType::Focus
        } else {
            BlockType::Normal
        }
    };

    // Get single mqtt_ctrl binding for the entire function to avoid lifetime issues
    with_mqtt_ctrl(|mqtt_ctrl| -> Result<(), DMError> {
        // Device Info
        {
            let device_info = mqtt_ctrl.device_info();

            // Device manifest
            draw_device_manifest(
                body_sub_chunks_left[3],
                buf,
                device_info,
                get_block_type(MainWindowFocus::DeviceManifest),
            )?;

            // main_chip
            draw_chip_info(
                body_sub_chunks_left[0],
                buf,
                device_info,
                "main_chip",
                get_block_type(MainWindowFocus::MainChip),
            )?;
            // companion_chip
            draw_chip_info(
                body_sub_chunks_left[1],
                buf,
                device_info,
                "companion_chip",
                get_block_type(MainWindowFocus::CompanionChip),
            )?;
            //sensor_chip
            draw_chip_info(
                body_sub_chunks_left[2],
                buf,
                device_info,
                "sensor_chip",
                get_block_type(MainWindowFocus::SensorChip),
            )?;
        }

        // Agent State
        let agent_system_info = mqtt_ctrl.agent_system_info();
        let agent_device_config = mqtt_ctrl.agent_device_config();
        draw_agent_state(
            body_sub_chunks_middle[0],
            buf,
            agent_system_info,
            agent_device_config,
            get_block_type(MainWindowFocus::AgentState),
        )?;

        // Deployment status
        let deployment_status = mqtt_ctrl.deployment_status();
        draw_deployment_status(
            body_sub_chunks_middle[1],
            buf,
            deployment_status,
            get_block_type(MainWindowFocus::DeploymentStatus),
        )?;

        // Reserved
        let device_reserved = mqtt_ctrl.device_reserved();
        draw_device_reserved(
            body_sub_chunks_middle[2],
            buf,
            device_reserved,
            get_block_type(MainWindowFocus::DeviceReserved),
        )?;

        // Device States
        let device_states = mqtt_ctrl.device_states();
        draw_device_states(
            body_sub_chunks_middle[3],
            buf,
            device_states,
            get_block_type(MainWindowFocus::DeviceState),
        )?;

        // Device Capabilities
        let device_capabilities = mqtt_ctrl.device_capabilities();
        draw_device_capabilities(
            body_sub_chunks_middle[4],
            buf,
            device_capabilities,
            get_block_type(MainWindowFocus::DeviceCapabilities),
        )?;

        //System Settings
        let system_settings = mqtt_ctrl.system_settings();
        draw_system_settings(
            body_sub_chunks_right[0],
            buf,
            system_settings,
            get_block_type(MainWindowFocus::SystemSettings),
        )?;

        // NetworkSettings
        let network_settings = mqtt_ctrl.network_settings();
        draw_network_settings(
            body_sub_chunks_right[1],
            buf,
            network_settings,
            get_block_type(MainWindowFocus::NetworkSettings),
        )?;

        // Wireless Settings
        let wireless_settings = mqtt_ctrl.wireless_settings();
        draw_wireless_settings(
            body_sub_chunks_right[2],
            buf,
            wireless_settings,
            get_block_type(MainWindowFocus::WirelessSettings),
        )?;

        Ok(())
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
        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);

        // draw() calls with_mqtt_ctrl which will panic when the global MqttCtrl is not initialized.
        let _ = draw(area, &mut buf, &app);
    }
}
