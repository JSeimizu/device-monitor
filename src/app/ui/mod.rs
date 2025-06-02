pub mod ui_config;
pub mod ui_config_user;
pub mod ui_directcmd;
pub mod ui_exit;
pub mod ui_foot;
pub mod ui_head;
pub mod ui_main;
pub mod ui_module;

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

pub fn list_items_push_focus(list_items: &mut Vec<ListItem>, name: &str, value: &str, focus: bool) {
    if focus {
        list_items.push(ListItem::new(Span::styled(
            format!("{:<25} : {}", name, value),
            Style::default().bg(Color::Gray).fg(Color::Black),
        )));
    } else {
        list_items.push(ListItem::new(Span::styled(
            format!("{:<25} : {}", name, value),
            Style::default(),
        )));
    }
}

pub fn list_items_push(list_items: &mut Vec<ListItem>, name: &str, value: &str) {
    list_items.push(ListItem::new(Span::styled(
        format!("{:<25} : {}", name, value),
        Style::default(),
    )));
}

pub fn draw_device_manifest(
    area: Rect,
    buf: &mut Buffer,
    device_info: &DeviceInfo,
    block_type: BlockType,
) -> Result<(), DMError> {
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

    Ok(())
}

pub fn draw_chip_info(
    area: Rect,
    buf: &mut Buffer,
    device_info: &DeviceInfo,
    chip_name: &str,
    block_type: BlockType,
) -> Result<(), DMError> {
    let mut list_items = Vec::<ListItem>::new();

    let chip = ChipInfo::new(chip_name)?;

    let mut r_chip = &chip;
    match chip_name {
        "main_chip" => {
            if let Some(main_chip) = device_info.main_chip() {
                r_chip = main_chip;
            }
        }
        "companion_chip" => {
            if let Some(chip) = device_info.companion_chip() {
                r_chip = chip;
            }
        }
        "sensor_chip" => {
            if let Some(chip) = device_info.sensor_chip() {
                r_chip = chip;
            }
        }
        _ => {}
    }

    list_items_push(&mut list_items, "id", r_chip.id());
    list_items_push(
        &mut list_items,
        "hardware_version",
        r_chip.hardware_version().unwrap_or(""),
    );
    list_items_push(
        &mut list_items,
        "temperature",
        r_chip.temperature().to_string().as_str(),
    );
    list_items_push(
        &mut list_items,
        "loader_version",
        r_chip.loader_version().unwrap_or(""),
    );
    list_items_push(
        &mut list_items,
        "loader_hash",
        r_chip.loader_hash().unwrap_or(""),
    );
    list_items_push(
        &mut list_items,
        "update_date_loader",
        r_chip.update_date_loader().unwrap_or(""),
    );

    list_items_push(
        &mut list_items,
        "firmware_version",
        r_chip.firmware_version().unwrap_or(""),
    );
    list_items_push(
        &mut list_items,
        "update_date_firmware",
        r_chip.update_date_firmware().unwrap_or(""),
    );

    for (i, model) in r_chip.ai_models().iter().enumerate() {
        list_items_push(
            &mut list_items,
            &format!("ai_model[{i}].version"),
            model.version(),
        );

        list_items_push(
            &mut list_items,
            &format!("ai_model[{i}].hash"),
            model.hash(),
        );
        list_items_push(
            &mut list_items,
            &format!("ai_model[{i}].update_date"),
            model.update_date(),
        );
    }

    let title = format!(" {} ", chip_name.replace("_", " ").to_uppercase());
    let mut block = normal_block(&title);
    if block_type == BlockType::Focus {
        block = focus_block(&title);
    }
    List::new(list_items).block(block).render(area, buf);
    Ok(())
}

pub fn draw_agent_state(
    area: Rect,
    buf: &mut Buffer,
    agent_system_info: &AgentSystemInfo,
    agent_device_config: &AgentDeviceConfig,
    block_type: BlockType,
) -> Result<(), DMError> {
    let mut list_items = Vec::<ListItem>::new();

    list_items_push(&mut list_items, "os", agent_system_info.os());
    list_items_push(&mut list_items, "arch", agent_system_info.arch());
    list_items_push(&mut list_items, "evp_agent", agent_system_info.evp_agent());

    if let Some(commit_hash) = agent_system_info.evp_agent_commit_hash() {
        list_items_push(&mut list_items, "evp_agent_commit_hash", commit_hash);
    }

    list_items_push(
        &mut list_items,
        "wasmMicroRuntime",
        agent_system_info.wasm_micro_runtime(),
    );

    list_items_push(
        &mut list_items,
        "protocolVersion",
        agent_system_info.protocol_version(),
    );

    list_items_push(
        &mut list_items,
        "report-status-interval-min",
        agent_device_config
            .report_status_interval_min
            .to_string()
            .as_str(),
    );

    list_items_push(
        &mut list_items,
        "report-status-interval-max",
        agent_device_config
            .report_status_interval_max
            .to_string()
            .as_str(),
    );

    let title = " AGENT STATE ";
    let mut block = normal_block(title);
    if block_type == BlockType::Focus {
        block = focus_block(title);
    }

    List::new(list_items).block(block).render(area, buf);

    Ok(())
}

pub fn draw_deployment_status(
    area: Rect,
    buf: &mut Buffer,
    deployment_status: &DeploymentStatus,
    block_type: BlockType,
) -> Result<(), DMError> {
    let mut list_items = Vec::<ListItem>::new();

    for (k, (uuid, instance)) in deployment_status.instances().iter().enumerate() {
        list_items_push(
            &mut list_items,
            &format!("instance[{}].uuid", k),
            uuid.uuid(),
        );

        list_items_push(
            &mut list_items,
            &format!("instance[{}].status", k),
            instance.status(),
        );

        list_items_push(
            &mut list_items,
            &format!("instance[{}].module_id", k),
            instance.module_id(),
        );

        list_items_push(
            &mut list_items,
            &format!("instance[{}].failure_message", k),
            instance.failure_message().unwrap_or(""),
        );
    }

    for (k, (uuid, module)) in deployment_status.modules().iter().enumerate() {
        list_items_push(&mut list_items, &format!("module[{}].uuid", k), uuid.uuid());

        list_items_push(
            &mut list_items,
            &format!("module[{}].status", k),
            module.status(),
        );

        list_items_push(
            &mut list_items,
            &format!("module[{}].failure_message", k),
            module.failure_message().unwrap_or(""),
        );
    }

    list_items_push(
        &mut list_items,
        "deployment_id",
        deployment_status
            .deployment_id()
            .map(|a| a.uuid())
            .unwrap_or_default(),
    );

    list_items_push(
        &mut list_items,
        "reconcile_status",
        deployment_status.reconcile_status().unwrap_or_default(),
    );

    let title = " DEPLOYMENT STATUS ";
    let mut block = normal_block(title);
    if block_type == BlockType::Focus {
        block = focus_block(title);
    }

    List::new(list_items).block(block).render(area, buf);

    Ok(())
}

pub fn draw_device_reserved(
    area: Rect,
    buf: &mut Buffer,
    device_reserved: &DeviceReserved,
    block_type: BlockType,
) -> Result<(), DMError> {
    let mut list_items = Vec::<ListItem>::new();
    let device_reserved_parsed = device_reserved.parse().unwrap_or_default();

    list_items_push(&mut list_items, "device", device_reserved_parsed.device);

    list_items_push(
        &mut list_items,
        "version",
        device_reserved_parsed.dtmi_version.to_string().as_str(),
    );

    list_items_push(
        &mut list_items,
        "dtmi_path",
        device_reserved_parsed.dtmi_path,
    );

    let title = " DEVICE RESERVED ";
    let mut block = normal_block(title);
    if block_type == BlockType::Focus {
        block = focus_block(title);
    }

    List::new(list_items).block(block).render(area, buf);

    Ok(())
}

pub fn draw_device_states(
    area: Rect,
    buf: &mut Buffer,
    device_states: &DeviceStates,
    block_type: BlockType,
) -> Result<(), DMError> {
    let mut list_items = Vec::<ListItem>::new();

    list_items_push(
        &mut list_items,
        "power_sources",
        device_states.power_state().power_sources().as_str(),
    );
    list_items_push(
        &mut list_items,
        "power_source_in_use",
        device_states.power_state().power_sources_in_use().as_str(),
    );
    list_items_push(
        &mut list_items,
        "is_battery_low",
        device_states
            .power_state()
            .is_battery_low()
            .to_string()
            .as_str(),
    );
    list_items_push(
        &mut list_items,
        "process_state",
        device_states.process_state(),
    );
    list_items_push(
        &mut list_items,
        "hours_meter",
        device_states.hours_meter().to_string().as_str(),
    );
    list_items_push(
        &mut list_items,
        "bootup_reason",
        device_states.bootup_reason().as_str(),
    );
    list_items_push(
        &mut list_items,
        "last_bootup_time",
        device_states.last_bootup_time(),
    );

    let title = " DEVICE STATE ";
    let mut block = normal_block(title);
    if block_type == BlockType::Focus {
        block = focus_block(title);
    }
    List::new(list_items).block(block).render(area, buf);
    Ok(())
}

pub fn draw_device_capabilities(
    area: Rect,
    buf: &mut Buffer,
    device_capabilities: &DeviceCapabilities,
    block_type: BlockType,
) -> Result<(), DMError> {
    let mut list_items = Vec::<ListItem>::new();

    list_items_push(
        &mut list_items,
        "is_battery_supported",
        device_capabilities
            .is_battery_supported()
            .to_string()
            .as_str(),
    );
    list_items_push(
        &mut list_items,
        "supported_wireless_mode",
        device_capabilities.supported_wireless_mode().as_str(),
    );
    list_items_push(
        &mut list_items,
        "is_periodic_supported",
        device_capabilities
            .is_periodic_supported()
            .to_string()
            .as_str(),
    );
    list_items_push(
        &mut list_items,
        "is_sensor_postprocess_supported",
        device_capabilities
            .is_sensor_postprocess_supported()
            .to_string()
            .as_str(),
    );

    let title = " DEVICE CAPABILITIES ";
    let mut block = normal_block(title);
    if block_type == BlockType::Focus {
        block = focus_block(title);
    }
    List::new(list_items).block(block).render(area, buf);

    Ok(())
}

pub fn draw_system_settings(
    area: Rect,
    buf: &mut Buffer,
    system_settings: &SystemSettings,
    block_type: BlockType,
) -> Result<(), DMError> {
    let mut list_items = Vec::<ListItem>::new();

    list_items_push(
        &mut list_items,
        "req_info.req_id",
        system_settings.req_info().req_id(),
    );

    list_items_push(
        &mut list_items,
        "led_enabled",
        system_settings.led_enabled().to_string().as_str(),
    );

    list_items_push(
        &mut list_items,
        "temperature_update_interval",
        system_settings
            .temperature_update_interval()
            .to_string()
            .as_str(),
    );

    for l in system_settings.log_settings().iter() {
        let filter = l.filter();
        list_items_push(
            &mut list_items,
            &format!("log.{}.level", filter),
            &format!("{}({})", l.level_str(), l.level()),
        );
        list_items_push(
            &mut list_items,
            &format!("log.{}.destination", filter),
            &format!("{}({})", l.destination_str(), l.destination()),
        );
        list_items_push(
            &mut list_items,
            &format!("log.{}.storage_name", filter),
            l.storage_name(),
        );
        list_items_push(
            &mut list_items,
            &format!("log.{}.path", filter),
            l.path().to_owned().as_str(),
        );
    }

    list_items_push(
        &mut list_items,
        "res_info.res_id",
        system_settings.res_info().res_id(),
    );
    list_items_push(
        &mut list_items,
        "res_info.code",
        system_settings.res_info().code_str(),
    );
    list_items_push(
        &mut list_items,
        "res_info.detail_msg",
        system_settings.res_info().detail_msg(),
    );

    let title = " SYSTEM SETTINGS ";
    let mut block = normal_block(title);
    if block_type == BlockType::Focus {
        block = focus_block(title);
    }

    List::new(list_items).block(block).render(area, buf);

    Ok(())
}

pub fn draw_network_settings(
    area: Rect,
    buf: &mut Buffer,
    network_settings: &NetworkSettings,
    block_type: BlockType,
) -> Result<(), DMError> {
    let mut list_items = Vec::<ListItem>::new();
    list_items_push(
        &mut list_items,
        "req_info.req_id",
        network_settings.req_info().req_id(),
    );

    let ip_method = network_settings.ip_method();
    let is_static = ip_method == "static";

    list_items_push(&mut list_items, "ip_method", ip_method);
    list_items_push(&mut list_items, "ntp_url", network_settings.ntp_url());

    if is_static {
        if let Some(ipv4) = network_settings.ipv4() {
            list_items_push(&mut list_items, "ipv4_address", ipv4.ip_address());
            list_items_push(&mut list_items, "ipv4_subnet_mask", ipv4.subnet_mask());
            list_items_push(&mut list_items, "ipv4_gateway", ipv4.gateway());
            list_items_push(&mut list_items, "ipv4_dns", ipv4.dns());
        }

        if let Some(ipv6) = network_settings.ipv6() {
            list_items_push(&mut list_items, "ipv6_address", ipv6.ip_address());
            list_items_push(&mut list_items, "ipv6_subnet_mask", ipv6.subnet_mask());
            list_items_push(&mut list_items, "ipv6_gateway", ipv6.gateway());
            list_items_push(&mut list_items, "ipv6_dns", ipv6.dns());
        }
    }

    if let Some(proxy_settings) = network_settings.proxy() {
        list_items_push(&mut list_items, "proxy_url", proxy_settings.url());
        list_items_push(
            &mut list_items,
            "proxy_port",
            proxy_settings.port().to_string().as_str(),
        );
        if let Some(user_name) = proxy_settings.user_name() {
            list_items_push(&mut list_items, "proxy_user_name", user_name);
        }
        if let Some(password) = proxy_settings.password() {
            list_items_push(&mut list_items, "proxy_password", password);
        }
    }

    list_items_push(
        &mut list_items,
        "res_info.res_id",
        network_settings.res_info().res_id(),
    );

    list_items_push(
        &mut list_items,
        "res_info.code",
        network_settings.res_info().code_str(),
    );

    list_items_push(
        &mut list_items,
        "res_info.detail_msg",
        network_settings.res_info().detail_msg(),
    );

    let title = " NETWORK SETTINGS ";
    let mut block = normal_block(title);
    if block_type == BlockType::Focus {
        block = focus_block(title);
    }

    List::new(list_items).block(block).render(area, buf);

    Ok(())
}

pub fn draw_wireless_settings(
    area: Rect,
    buf: &mut Buffer,
    wireless_settings: &WirelessSettings,
    block_type: BlockType,
) -> Result<(), DMError> {
    let mut list_items = Vec::<ListItem>::new();

    list_items_push(
        &mut list_items,
        "req_info.req_id",
        wireless_settings.req_info().req_id(),
    );

    let station_setting = wireless_settings.sta_mode_setting();
    list_items_push(&mut list_items, "sta.ssid", station_setting.ssid());
    list_items_push(&mut list_items, "sta.password", station_setting.password());

    list_items_push(
        &mut list_items,
        "sta.encryption",
        station_setting.encryption(),
    );

    list_items_push(
        &mut list_items,
        "res_info.res_id",
        wireless_settings.res_info().res_id(),
    );

    list_items_push(
        &mut list_items,
        "res_info.code",
        wireless_settings.res_info().code_str(),
    );

    list_items_push(
        &mut list_items,
        "res_info.detail_msg",
        wireless_settings.res_info().detail_msg(),
    );

    let title = " WIRELESS SETTINGS ";
    let mut block = normal_block(title);
    if block_type == BlockType::Focus {
        block = focus_block(title);
    }

    List::new(list_items).block(block).render(area, buf);

    Ok(())
}
