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

pub mod ui_config;
pub mod ui_config_user;
pub mod ui_directcmd;
pub mod ui_edge_app;
pub mod ui_elog;
pub mod ui_evp_module;
pub mod ui_exit;
pub mod ui_foot;
pub mod ui_head;
pub mod ui_main;
pub mod ui_module;
pub mod ui_ota;
pub mod ui_ota_config;
pub mod ui_token_provider;
pub mod ui_token_provider_blobs;

#[allow(unused)]
use {
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
    base64::{Engine as _, engine::general_purpose},
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    error_stack::{Report, Result},
    jlogger_tracing::{JloggerBuilder, LevelFilter, LogTimeFormat, jdebug, jerror, jinfo},
    ratatui::symbols,
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
    serde_json::Value,
    std::{
        collections::HashMap,
        io,
        time::{Duration, Instant},
    },
};

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[derive(Debug, PartialEq, Eq)]
pub enum BlockType {
    Normal,
    Focus,
}

pub fn normal_block(title: &str) -> Block<'_> {
    Block::default()
        .title(Span::styled(title, Style::new().fg(Color::Yellow)))
        .borders(Borders::ALL)
}

pub fn focus_block(title: &str) -> Block<'_> {
    Block::default()
        .title(Span::styled(
            title,
            Style::new().fg(Color::LightYellow).bold(),
        ))
        .borders(Borders::ALL)
        .border_set(symbols::border::THICK)
        .bold()
}

pub fn list_items_push_text_focus(list_items: &mut Vec<ListItem>, value: &str, focus: bool) {
    if focus {
        list_items.push(ListItem::new(Span::styled(
            value.to_owned(),
            Style::default().bg(Color::Gray).fg(Color::Black),
        )));
    } else {
        list_items.push(ListItem::new(Span::styled(
            value.to_owned(),
            Style::default(),
        )));
    }
}

pub fn list_items_push_focus(list_items: &mut Vec<ListItem>, name: &str, value: &str, focus: bool) {
    list_items_push_text_focus(list_items, &format!("{:<35} : {}", name, value), focus);
}

pub fn list_items_push(list_items: &mut Vec<ListItem>, name: &str, value: &str) {
    list_items.push(ListItem::new(Span::styled(
        format!("{:<35} : {}", name, value),
        Style::default(),
    )));
}

pub fn list_items_push_blank(list_items: &mut Vec<ListItem>) {
    list_items.push(ListItem::new(Span::styled("", Style::default())));
}

pub fn list_items_push_dynamic(
    list_items: &mut Vec<ListItem>,
    width: usize,
    name: &str,
    value: &str,
) {
    list_items.push(ListItem::new(Span::styled(
        format!("{:<padding$} : {}", name, value, padding = width),
        Style::default(),
    )));
}

pub fn draw_device_manifest(
    area: Rect,
    buf: &mut Buffer,
    device_info: Option<&DeviceInfo>,
    block_type: BlockType,
) -> Result<(), DMError> {
    if let Some(device_info) = device_info {
        let mut device_manifest_str = String::new();
        if let Some(s) = device_info.device_manifest() {
            let parts: Vec<&str> = s.split('.').collect();
            jdebug!(func = "draw_device_manifest", manifest = s);
            jdebug!(func = "draw_device_manifest", parts = parts.len());
            if parts.len() != 3 {
                device_manifest_str = "Invalid JWT".to_owned();
            } else {
                let decode_part = |part: &str| {
                    general_purpose::URL_SAFE_NO_PAD
                        .decode(part)
                        .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
                };

                let pretty_string = |json_str: &str| {
                    let json_value: Value = serde_json::from_str(json_str).unwrap_or_default();
                    serde_json::to_string_pretty(&json_value)
                        .unwrap_or_else(|_| "Invalid JSON".to_owned())
                };

                let mut header = String::new();
                if let Ok(h) = decode_part(parts[0]) {
                    header = pretty_string(&h);
                }

                let mut payload = String::new();
                if let Ok(p) = decode_part(parts[1]) {
                    payload = pretty_string(&p);
                }

                let signature = parts[2].to_string();
                if !header.is_empty() && !payload.is_empty() {
                    device_manifest_str = format!(
                        "Header:\n{}\n\nPayload:\n{}\n\nSignature:\n{{\n    {}\n}}",
                        header, payload, signature
                    );
                } else {
                    device_manifest_str = "Invalid JWT".to_owned();
                }
            }
        }

        let title = " DEVICE MANIFEST ";
        let block = match block_type {
            BlockType::Normal => normal_block(title),
            BlockType::Focus => focus_block(title),
        };

        Paragraph::new(device_manifest_str)
            .block(block)
            .render(area, buf);
    } else {
        let title = " DEVICE MANIFEST ";
        let block = match block_type {
            BlockType::Normal => normal_block(title),
            BlockType::Focus => focus_block(title),
        };
        Paragraph::new("No data available")
            .block(block)
            .render(area, buf);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::widgets::ListItem;

    #[test]
    fn test_centered_rect() {
        // Outer rect 100x40, request 50% x and y -> inner rect should be 50x20 centered at (25,10)
        let outer = Rect::new(0, 0, 100, 40);
        let r = centered_rect(50, 50, outer);
        assert_eq!(r.width, 50);
        assert_eq!(r.height, 20);
        assert_eq!(r.x, 25);
        assert_eq!(r.y, 10);
    }

    #[test]
    fn test_blocktype_and_block_factories() {
        // Ensure BlockType variants compare correctly and block factories run without panic.
        assert_ne!(BlockType::Normal, BlockType::Focus);

        // Call the block factory functions to ensure they compile and return a Block.
        let _n = normal_block("TITLE");
        let _f = focus_block("TITLE");
    }

    #[test]
    fn test_list_items_push_variants() {
        let mut list_items: Vec<ListItem> = Vec::new();

        // text focus true and false should append items
        list_items_push_text_focus(&mut list_items, "focused", true);
        list_items_push_text_focus(&mut list_items, "not_focused", false);

        // key/value push
        list_items_push(&mut list_items, "name", "value");

        // blank push
        list_items_push_blank(&mut list_items);

        // dynamic push with padding
        list_items_push_dynamic(&mut list_items, 10, "dyn", "val");

        assert_eq!(list_items.len(), 5);
    }

    #[test]
    fn test_list_items_push_focus_shortcut() {
        let mut list_items: Vec<ListItem> = Vec::new();
        // Uses list_items_push_focus which delegates to list_items_push_text_focus
        list_items_push_focus(&mut list_items, "nm", "v", true);
        list_items_push_focus(&mut list_items, "nm2", "v2", false);

        assert_eq!(list_items.len(), 2);
    }
}

pub fn draw_chip_info(
    area: Rect,
    buf: &mut Buffer,
    device_info: Option<&DeviceInfo>,
    chip_name: &str,
    block_type: BlockType,
) -> Result<(), DMError> {
    if let Some(device_info) = device_info {
        let mut list_items = Vec::<ListItem>::new();
        let width = 20;

        let chip = match chip_name {
            "main_chip" => device_info.main_chip(),
            "companion_chip" => device_info.companion_chip(),
            "sensor_chip" => device_info.sensor_chip(),
            _ => {
                return Err(Report::new(DMError::InvalidData)
                    .attach_printable(format!("Unknown chip name: {}", chip_name)));
            }
        };

        if let Some(r_chip) = chip {
            list_items_push_dynamic(&mut list_items, width, "id", r_chip.id());
            list_items_push_dynamic(
                &mut list_items,
                width,
                "hardware_version",
                r_chip.hardware_version().unwrap_or(""),
            );
            list_items_push_dynamic(
                &mut list_items,
                width,
                "temperature",
                r_chip.temperature().to_string().as_str(),
            );
            list_items_push_dynamic(
                &mut list_items,
                width,
                "loader_version",
                r_chip.loader_version().unwrap_or(""),
            );
            list_items_push_dynamic(
                &mut list_items,
                width,
                "loader_hash",
                r_chip.loader_hash().unwrap_or(""),
            );
            list_items_push_dynamic(
                &mut list_items,
                width,
                "update_date_loader",
                r_chip.update_date_loader().unwrap_or(""),
            );

            list_items_push_dynamic(
                &mut list_items,
                width,
                "firmware_version",
                r_chip.firmware_version().unwrap_or(""),
            );
            list_items_push_dynamic(
                &mut list_items,
                width,
                "update_date_firmware",
                r_chip.update_date_firmware().unwrap_or(""),
            );

            for (i, model) in r_chip.ai_models().iter().enumerate() {
                list_items_push_text_focus(&mut list_items, &format!("ai_model[{}]", i), false);
                list_items_push_dynamic(&mut list_items, width, "  version", model.version());

                list_items_push_dynamic(&mut list_items, width, "  hash", model.hash());
                list_items_push_dynamic(
                    &mut list_items,
                    width,
                    "  update_date",
                    model.update_date(),
                );
            }

            let title = format!(" {} ", chip_name.replace("_", " ").to_uppercase());
            let block = match block_type {
                BlockType::Normal => normal_block(&title),
                BlockType::Focus => focus_block(&title),
            };
            List::new(list_items).block(block).render(area, buf);
        } else {
            let title = format!(" {} ", chip_name.replace("_", " ").to_uppercase());
            let block = match block_type {
                BlockType::Normal => normal_block(&title),
                BlockType::Focus => focus_block(&title),
            };
            Paragraph::new("No data available")
                .block(block)
                .render(area, buf);
        }
    } else {
        let title = format!(" {} ", chip_name.replace("_", " ").to_uppercase());
        let block = match block_type {
            BlockType::Normal => normal_block(&title),
            BlockType::Focus => focus_block(&title),
        };
        Paragraph::new("No data available")
            .block(block)
            .render(area, buf);
    }
    Ok(())
}

pub fn draw_agent_state(
    area: Rect,
    buf: &mut Buffer,
    agent_system_info: Option<&AgentSystemInfo>,
    agent_device_config: Option<&AgentDeviceConfig>,
    block_type: BlockType,
) -> Result<(), DMError> {
    if let (Some(agent_system_info), Some(agent_device_config)) =
        (agent_system_info, agent_device_config)
    {
        let mut list_items = Vec::<ListItem>::new();
        let width = 27;

        list_items_push_dynamic(&mut list_items, width, "os", agent_system_info.os());
        list_items_push_dynamic(&mut list_items, width, "arch", agent_system_info.arch());
        list_items_push_dynamic(
            &mut list_items,
            width,
            "evp_agent",
            agent_system_info.evp_agent(),
        );

        if let Some(commit_hash) = agent_system_info.evp_agent_commit_hash() {
            list_items_push_dynamic(&mut list_items, width, "evp_agent_commit_hash", commit_hash);
        }

        list_items_push_dynamic(
            &mut list_items,
            width,
            "wasmMicroRuntime",
            agent_system_info.wasm_micro_runtime(),
        );

        list_items_push_dynamic(
            &mut list_items,
            width,
            "protocolVersion",
            agent_system_info.protocol_version(),
        );

        list_items_push_dynamic(
            &mut list_items,
            width,
            "report-status-interval-min",
            agent_device_config
                .report_status_interval_min
                .to_string()
                .as_str(),
        );

        list_items_push_dynamic(
            &mut list_items,
            width,
            "report-status-interval-max",
            agent_device_config
                .report_status_interval_max
                .to_string()
                .as_str(),
        );

        let title = " AGENT STATE ";
        let block = match block_type {
            BlockType::Normal => normal_block(title),
            BlockType::Focus => focus_block(title),
        };

        List::new(list_items).block(block).render(area, buf);
    } else {
        let title = " AGENT STATE ";
        let block = match block_type {
            BlockType::Normal => normal_block(title),
            BlockType::Focus => focus_block(title),
        };
        Paragraph::new("No data available")
            .block(block)
            .render(area, buf);
    }

    Ok(())
}

pub fn draw_deployment_status(
    area: Rect,
    buf: &mut Buffer,
    deployment_status: Option<&DeploymentStatus>,
    block_type: BlockType,
) -> Result<(), DMError> {
    if let Some(deployment_status) = deployment_status {
        let mut list_items = Vec::<ListItem>::new();
        let width = 18;

        for (k, (uuid, instance)) in deployment_status.instances().iter().enumerate() {
            list_items_push_text_focus(&mut list_items, &format!("instance[{}]", k), false);
            list_items_push_dynamic(&mut list_items, width, "  uuid", uuid.uuid());
            list_items_push_dynamic(&mut list_items, width, "  status", instance.status());
            list_items_push_dynamic(&mut list_items, width, "  module_id", instance.module_id());

            list_items_push_dynamic(
                &mut list_items,
                width,
                "  failure_message",
                instance.failure_message().unwrap_or(""),
            );
        }

        for (k, (uuid, module)) in deployment_status.modules().iter().enumerate() {
            list_items_push_text_focus(&mut list_items, &format!("module[{}]", k), false);
            list_items_push_dynamic(&mut list_items, width, "  uuid", uuid.uuid());
            list_items_push_dynamic(&mut list_items, width, "  status", module.status());

            list_items_push_dynamic(
                &mut list_items,
                width,
                "  failure_message",
                module.failure_message().unwrap_or(""),
            );
        }

        list_items_push_dynamic(
            &mut list_items,
            width,
            "deployment_id",
            deployment_status
                .deployment_id()
                .map(|a| a.uuid())
                .unwrap_or_default(),
        );

        list_items_push_dynamic(
            &mut list_items,
            width,
            "reconcile_status",
            deployment_status.reconcile_status().unwrap_or_default(),
        );

        let title = " DEPLOYMENT STATUS ";
        let block = match block_type {
            BlockType::Normal => normal_block(title),
            BlockType::Focus => focus_block(title),
        };

        List::new(list_items).block(block).render(area, buf);
    } else {
        let title = " DEPLOYMENT STATUS ";
        let block = match block_type {
            BlockType::Normal => normal_block(title),
            BlockType::Focus => focus_block(title),
        };
        Paragraph::new("No data available")
            .block(block)
            .render(area, buf);
    }

    Ok(())
}

pub fn draw_device_reserved(
    area: Rect,
    buf: &mut Buffer,
    device_reserved: Option<&DeviceReserved>,
    block_type: BlockType,
) -> Result<(), DMError> {
    if let Some(device_reserved) = device_reserved {
        let mut list_items = Vec::<ListItem>::new();
        let device_reserved_parsed = device_reserved.parse().unwrap_or_default();
        let width = 10;

        list_items_push_dynamic(
            &mut list_items,
            width,
            "device",
            device_reserved_parsed.device,
        );

        list_items_push_dynamic(
            &mut list_items,
            width,
            "version",
            device_reserved_parsed.dtmi_version.to_string().as_str(),
        );

        list_items_push_dynamic(
            &mut list_items,
            width,
            "dtmi_path",
            device_reserved_parsed.dtmi_path,
        );

        let title = " DEVICE RESERVED ";
        let block = match block_type {
            BlockType::Normal => normal_block(title),
            BlockType::Focus => focus_block(title),
        };

        List::new(list_items).block(block).render(area, buf);
    } else {
        let title = " DEVICE RESERVED ";
        let block = match block_type {
            BlockType::Normal => normal_block(title),
            BlockType::Focus => focus_block(title),
        };
        Paragraph::new("No data available")
            .block(block)
            .render(area, buf);
    }

    Ok(())
}

pub fn draw_device_states(
    area: Rect,
    buf: &mut Buffer,
    device_states: Option<&DeviceStates>,
    block_type: BlockType,
) -> Result<(), DMError> {
    if let Some(device_states) = device_states {
        let mut list_items = Vec::<ListItem>::new();
        let width = 20;

        list_items_push_dynamic(
            &mut list_items,
            width,
            "power_sources",
            device_states.power_state().power_sources().as_str(),
        );
        list_items_push_dynamic(
            &mut list_items,
            width,
            "power_source_in_use",
            device_states.power_state().power_sources_in_use().as_str(),
        );
        list_items_push_dynamic(
            &mut list_items,
            width,
            "is_battery_low",
            device_states
                .power_state()
                .is_battery_low()
                .to_string()
                .as_str(),
        );
        list_items_push_dynamic(
            &mut list_items,
            width,
            "process_state",
            device_states.process_state(),
        );
        list_items_push_dynamic(
            &mut list_items,
            width,
            "hours_meter",
            device_states.hours_meter().to_string().as_str(),
        );
        list_items_push_dynamic(
            &mut list_items,
            width,
            "bootup_reason",
            device_states.bootup_reason().as_str(),
        );
        list_items_push_dynamic(
            &mut list_items,
            width,
            "last_bootup_time",
            device_states.last_bootup_time(),
        );

        let title = " DEVICE STATE ";
        let block = match block_type {
            BlockType::Normal => normal_block(title),
            BlockType::Focus => focus_block(title),
        };
        List::new(list_items).block(block).render(area, buf);
    } else {
        let title = " DEVICE STATE ";
        let block = match block_type {
            BlockType::Normal => normal_block(title),
            BlockType::Focus => focus_block(title),
        };
        Paragraph::new("No data available")
            .block(block)
            .render(area, buf);
    }
    Ok(())
}

pub fn draw_device_capabilities(
    area: Rect,
    buf: &mut Buffer,
    device_capabilities: Option<&DeviceCapabilities>,
    block_type: BlockType,
) -> Result<(), DMError> {
    if let Some(device_capabilities) = device_capabilities {
        let mut list_items = Vec::<ListItem>::new();
        let width = 31;

        if let Some(v) = device_capabilities.is_battery_supported() {
            list_items_push_dynamic(
                &mut list_items,
                width,
                "is_battery_supported",
                v.to_string().as_str(),
            );
        }

        if let Some(v) = device_capabilities.supported_wireless_mode() {
            list_items_push_dynamic(
                &mut list_items,
                width,
                "supported_wireless_mode",
                v.as_str(),
            );
        }

        if let Some(v) = device_capabilities.is_periodic_supported() {
            list_items_push_dynamic(
                &mut list_items,
                width,
                "is_periodic_supported",
                v.to_string().as_str(),
            );
        }

        if let Some(v) = device_capabilities.is_sensor_postprocess_supported() {
            list_items_push_dynamic(
                &mut list_items,
                width,
                "is_sensor_postprocess_supported",
                v.to_string().as_str(),
            );
        }

        let title = " DEVICE CAPABILITIES ";
        let block = match block_type {
            BlockType::Normal => normal_block(title),
            BlockType::Focus => focus_block(title),
        };
        List::new(list_items).block(block).render(area, buf);
    } else {
        let title = " DEVICE CAPABILITIES ";
        let block = match block_type {
            BlockType::Normal => normal_block(title),
            BlockType::Focus => focus_block(title),
        };
        Paragraph::new("No data available")
            .block(block)
            .render(area, buf);
    }

    Ok(())
}

pub fn draw_system_settings(
    area: Rect,
    buf: &mut Buffer,
    system_settings: Option<&SystemSettings>,
    block_type: BlockType,
) -> Result<(), DMError> {
    if let Some(system_settings) = system_settings {
        let mut list_items = Vec::<ListItem>::new();
        let width = 12;

        list_items_push_text_focus(&mut list_items, "req_info", false);
        list_items_push_dynamic(
            &mut list_items,
            width,
            "  req_id",
            system_settings.req_info().req_id(),
        );

        if let Some(led_enabled) = system_settings.led_enabled() {
            list_items_push_dynamic(
                &mut list_items,
                width,
                "led_enabled",
                led_enabled.to_string().as_str(),
            );
        }

        if let Some(temperature_update_interval) = system_settings.temperature_update_interval() {
            list_items_push_dynamic(
                &mut list_items,
                width,
                "temperature_update_interval",
                temperature_update_interval.to_string().as_str(),
            );
        }

        if let Some(log_settings) = system_settings.log_settings() {
            let width = 28;
            list_items_push_text_focus(&mut list_items, "log", false);
            for l in log_settings.iter() {
                let filter = l.filter();
                list_items_push_dynamic(
                    &mut list_items,
                    width,
                    &format!("  {}.level", filter),
                    &format!("{}({})", l.level_str(), l.level()),
                );
                list_items_push_dynamic(
                    &mut list_items,
                    width,
                    &format!("  {}.destination", filter),
                    &format!("{}({})", l.destination_str(), l.destination()),
                );
                list_items_push_dynamic(
                    &mut list_items,
                    width,
                    &format!("  {}.storage_name", filter),
                    l.storage_name(),
                );
                list_items_push_dynamic(
                    &mut list_items,
                    width,
                    &format!("  {}.path", filter),
                    l.path().to_owned().as_str(),
                );
            }
        }

        list_items_push_text_focus(&mut list_items, "res_info", false);
        list_items_push_dynamic(
            &mut list_items,
            width,
            "  res_id",
            system_settings.res_info().res_id(),
        );
        list_items_push_dynamic(
            &mut list_items,
            width,
            "  code",
            system_settings.res_info().code_str(),
        );
        list_items_push_dynamic(
            &mut list_items,
            width,
            "  detail_msg",
            system_settings.res_info().detail_msg(),
        );

        let title = " SYSTEM SETTINGS ";
        let block = match block_type {
            BlockType::Normal => normal_block(title),
            BlockType::Focus => focus_block(title),
        };

        List::new(list_items).block(block).render(area, buf);
    } else {
        let title = " SYSTEM SETTINGS ";
        let block = match block_type {
            BlockType::Normal => normal_block(title),
            BlockType::Focus => focus_block(title),
        };
        Paragraph::new("No data available")
            .block(block)
            .render(area, buf);
    }

    Ok(())
}

pub fn draw_network_settings(
    area: Rect,
    buf: &mut Buffer,
    network_settings: Option<&NetworkSettings>,
    block_type: BlockType,
) -> Result<(), DMError> {
    if let Some(network_settings) = network_settings {
        let mut list_items = Vec::<ListItem>::new();
        let width = 13;

        list_items_push_text_focus(&mut list_items, "req_info", false);
        list_items_push_dynamic(
            &mut list_items,
            width,
            "  req_id",
            network_settings.req_info().req_id(),
        );

        let ip_method = network_settings.ip_method();
        let is_static = ip_method == "static";

        list_items_push_dynamic(&mut list_items, width, "ip_method", ip_method);
        list_items_push_dynamic(
            &mut list_items,
            width,
            "ntp_url",
            network_settings.ntp_url(),
        );

        if is_static {
            if let Some(ipv4) = network_settings.ipv4() {
                list_items_push_text_focus(&mut list_items, "ipv4", false);
                list_items_push_dynamic(&mut list_items, width, "  address", ipv4.ip_address());
                list_items_push_dynamic(
                    &mut list_items,
                    width,
                    "  subnet_mask",
                    ipv4.subnet_mask(),
                );
                list_items_push_dynamic(&mut list_items, width, "  gateway", ipv4.gateway());
                list_items_push_dynamic(&mut list_items, width, "  dns", ipv4.dns());
            }

            if let Some(ipv6) = network_settings.ipv6() {
                list_items_push_text_focus(&mut list_items, "ipv6", false);
                list_items_push_dynamic(&mut list_items, width, "  address", ipv6.ip_address());
                list_items_push_dynamic(
                    &mut list_items,
                    width,
                    "  subnet_mask",
                    ipv6.subnet_mask(),
                );
                list_items_push_dynamic(&mut list_items, width, "  gateway", ipv6.gateway());
                list_items_push_dynamic(&mut list_items, width, "  dns", ipv6.dns());
            }
        }

        if let Some(proxy_settings) = network_settings.proxy() {
            list_items_push_text_focus(&mut list_items, "proxy", false);
            list_items_push_dynamic(&mut list_items, width, "  url", proxy_settings.url());
            list_items_push_dynamic(
                &mut list_items,
                width,
                "  port",
                proxy_settings.port().to_string().as_str(),
            );
            if let Some(user_name) = proxy_settings.user_name() {
                list_items_push_dynamic(&mut list_items, width, "  user_name", user_name);
            }
            if let Some(password) = proxy_settings.password() {
                list_items_push_dynamic(&mut list_items, width, "  password", password);
            }
        }

        list_items_push_text_focus(&mut list_items, "res_info", false);
        list_items_push_dynamic(
            &mut list_items,
            width,
            "  res_id",
            network_settings.res_info().res_id(),
        );

        list_items_push_dynamic(
            &mut list_items,
            width,
            "  code",
            network_settings.res_info().code_str(),
        );

        list_items_push_dynamic(
            &mut list_items,
            width,
            "  detail_msg",
            network_settings.res_info().detail_msg(),
        );

        let title = " NETWORK SETTINGS ";
        let block = match block_type {
            BlockType::Normal => normal_block(title),
            BlockType::Focus => focus_block(title),
        };

        List::new(list_items).block(block).render(area, buf);
    } else {
        let title = " NETWORK SETTINGS ";
        let block = match block_type {
            BlockType::Normal => normal_block(title),
            BlockType::Focus => focus_block(title),
        };
        Paragraph::new("No data available")
            .block(block)
            .render(area, buf);
    }

    Ok(())
}

pub fn draw_wireless_settings(
    area: Rect,
    buf: &mut Buffer,
    wireless_settings: Option<&WirelessSettings>,
    block_type: BlockType,
) -> Result<(), DMError> {
    if let Some(wireless_settings) = wireless_settings {
        let mut list_items = Vec::<ListItem>::new();
        let width = 12;

        list_items_push_text_focus(&mut list_items, "req_info", false);
        list_items_push_dynamic(
            &mut list_items,
            width,
            "  req_id",
            wireless_settings.req_info().req_id(),
        );

        if let Some(station_setting) = wireless_settings.sta_mode_setting() {
            list_items_push_text_focus(&mut list_items, "station", false);
            list_items_push_dynamic(&mut list_items, width, "  ssid", station_setting.ssid());
            list_items_push_dynamic(
                &mut list_items,
                width,
                "  password",
                station_setting.password(),
            );

            list_items_push_dynamic(
                &mut list_items,
                width,
                "  encryption",
                station_setting.encryption(),
            );
        }

        list_items_push_text_focus(&mut list_items, "res_info", false);
        list_items_push_dynamic(
            &mut list_items,
            width,
            "  res_id",
            wireless_settings.res_info().res_id(),
        );

        list_items_push_dynamic(
            &mut list_items,
            width,
            "  code",
            wireless_settings.res_info().code_str(),
        );

        list_items_push_dynamic(
            &mut list_items,
            width,
            "  detail_msg",
            wireless_settings.res_info().detail_msg(),
        );

        let title = " WIRELESS SETTINGS ";
        let block = match block_type {
            BlockType::Normal => normal_block(title),
            BlockType::Focus => focus_block(title),
        };

        List::new(list_items).block(block).render(area, buf);
    } else {
        let title = " WIRELESS SETTINGS ";
        let block = match block_type {
            BlockType::Normal => normal_block(title),
            BlockType::Focus => focus_block(title),
        };
        Paragraph::new("No data available")
            .block(block)
            .render(area, buf);
    }

    Ok(())
}
