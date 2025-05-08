use chrono::Local;
#[allow(unused)]
use {
    crate::{
        app::{App, CurrentScreen, CurrentlyEditing},
        error::DMError,
        mqtt_ctrl::{
            MqttCtrl,
            evp::device_info::{ChipInfo, DeviceInfo},
            evp::evp_state::{AgentDeviceConfig, AgentSystemInfo, UUID},
        },
    },
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
    let normal_block = |title: String| {
        Block::default()
            .title(Span::styled(title, Style::new().fg(Color::Yellow)))
            .borders(Borders::ALL)
    };

    let focus_block = |title: String| {
        Block::default()
            .title(Span::styled(
                title,
                Style::new().fg(Color::LightYellow).bold(),
            ))
            .borders(Borders::ALL)
            .bold()
    };

    let list_items_push = |list_items: &mut Vec<ListItem>, name: &str, value: &Option<String>| {
        list_items.push(ListItem::new(Span::styled(
            format!("{:<25} : {}", name, value.as_deref().unwrap_or("-")),
            Style::default(),
        )));
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(30),
            Constraint::Length(1),
        ])
        .split(area);

    // Draw title
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    Paragraph::new(Text::styled(
        format!("Device Monitor v{VERSION}"),
        Style::default().fg(Color::White).bold(),
    ))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::empty()))
    .render(chunks[0], buf);

    // Draw body
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(chunks[1]);

    // Device Info
    {
        let device_info_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Percentage(10),
            ])
            .split(body_chunks[0]);

        let device_info = app.mqtt_ctrl().device_info();
        // Device manifest
        {
            let device_manifest = device_info.device_manifest().unwrap_or("-");

            Paragraph::new(device_manifest)
                .block(normal_block(" DEVICE MANIFEST ".to_owned()))
                .render(device_info_chunks[3], buf);
        }

        let mut create_list = |chip_name: &str, focus: bool| {
            let mut dev_info_chunk_index = 3;
            match chip_name {
                "main_chip" => {
                    dev_info_chunk_index = 0;
                }
                "companion_chip" => {
                    dev_info_chunk_index = 1;
                }
                "sensor_chip" => {
                    dev_info_chunk_index = 2;
                }
                _ => {}
            }

            if dev_info_chunk_index >= 3 {
                return;
            }

            let mut list_items = Vec::<ListItem>::new();

            let chip = ChipInfo {
                name: Some(chip_name.to_owned()),
                ..Default::default()
            };

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

            list_items_push(&mut list_items, "id", &r_chip.id);
            list_items_push(
                &mut list_items,
                "hardware_version",
                &r_chip.hardware_version,
            );
            list_items_push(
                &mut list_items,
                "temperature",
                &Some(r_chip.temperature.to_string()),
            );
            list_items_push(&mut list_items, "loader_version", &r_chip.loader_version);
            list_items_push(&mut list_items, "loader_hash", &r_chip.loader_hash);
            list_items_push(
                &mut list_items,
                "update_date_loader",
                &r_chip.update_date_loader,
            );
            list_items_push(
                &mut list_items,
                "firmware_version",
                &r_chip.firmware_version,
            );
            list_items_push(
                &mut list_items,
                "update_date_firmware",
                &r_chip.update_date_firmware,
            );

            for (key, value) in r_chip
                .ai_models_pairs()
                .iter()
                .map(|a| (a.0.as_str(), a.1.as_str()))
            {
                list_items_push(&mut list_items, key, &Some(value.to_owned()));
            }

            let title = format!(" {} ", chip_name.replace("_", " ").to_uppercase());
            let mut block = normal_block(title.clone());
            if focus {
                block = focus_block(title.clone());
            }

            List::new(list_items)
                .block(block)
                .render(device_info_chunks[dev_info_chunk_index], buf);
        };

        // main_chip
        create_list("main_chip", false);
        // companion_chip
        create_list("companion_chip", false);
        //sensor_chip
        create_list("sensor_chip", false);
    }

    let body_sub_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
            Constraint::Percentage(15),
        ])
        .split(body_chunks[1]);

    // Agent State
    {
        let mut list_items = Vec::<ListItem>::new();
        let agent_system_info = app.mqtt_ctrl().agent_system_info();
        let agent_device_config = app.mqtt_ctrl().agent_device_config();

        list_items_push(
            &mut list_items,
            "os",
            &Some(agent_system_info.os().to_owned()),
        );
        list_items_push(
            &mut list_items,
            "arch",
            &Some(agent_system_info.arch().to_owned()),
        );
        list_items_push(
            &mut list_items,
            "evp_agent",
            &Some(agent_system_info.evp_agent().to_owned()),
        );
        list_items_push(
            &mut list_items,
            "evp_agent_commit_hash",
            &agent_system_info
                .evp_agent_commit_hash()
                .map(|a| a.to_owned()),
        );
        list_items_push(
            &mut list_items,
            "wasmMicroRuntime",
            &Some(agent_system_info.wasm_micro_runtime().to_owned()),
        );
        list_items_push(
            &mut list_items,
            "protocolVersion",
            &Some(agent_system_info.protocol_version().to_owned()),
        );
        list_items_push(
            &mut list_items,
            "report-status-interval-min",
            &Some(agent_device_config.report_status_interval_min.to_string()),
        );
        list_items_push(
            &mut list_items,
            "report-status-interval-max",
            &Some(agent_device_config.report_status_interval_max.to_string()),
        );
        //        list_items_push(
        //            &mut list_items,
        //            "deploymentStatus",
        //            &agent_system_info.deploymentStatus,
        //        );

        List::new(list_items)
            .block(normal_block(" AGENT STATE ".to_owned()))
            .render(body_sub_chunks[0], buf);
    }

    // Deployment status
    {
        let mut list_items = Vec::<ListItem>::new();
        let deployment_status = app.mqtt_ctrl.agent_system_info().deployment_status();
        for (k, (uuid, instance)) in deployment_status.instances().iter().enumerate() {
            list_items_push(
                &mut list_items,
                &format!("instance[{}].uuid", k),
                &Some(uuid.uuid().to_owned()),
            );

            list_items_push(
                &mut list_items,
                &format!("instance[{}].status", k),
                &Some(instance.status().to_owned()),
            );

            list_items_push(
                &mut list_items,
                &format!("instance[{}].module_id", k),
                &Some(instance.module_id().to_owned()),
            );

            list_items_push(
                &mut list_items,
                &format!("instance[{}].failure_message", k),
                &Some(
                    instance
                        .failure_message()
                        .map(|a| a.to_owned())
                        .unwrap_or_default(),
                ),
            );
        }

        for (k, (uuid, module)) in deployment_status.modules().iter().enumerate() {
            list_items_push(
                &mut list_items,
                &format!("module[{}].uuid", k),
                &Some(uuid.uuid().to_owned()),
            );

            list_items_push(
                &mut list_items,
                &format!("module[{}].status", k),
                &Some(module.status().to_owned()),
            );

            list_items_push(
                &mut list_items,
                &format!("module[{}].failure_message", k),
                &Some(
                    module
                        .failure_message()
                        .map(|a| a.to_owned())
                        .unwrap_or_default(),
                ),
            );
        }

        list_items_push(
            &mut list_items,
            "development_id",
            &Some(
                deployment_status
                    .deployment_id()
                    .map(|a| a.uuid().to_owned())
                    .unwrap_or_default(),
            ),
        );

        list_items_push(
            &mut list_items,
            "reconcile_status",
            &Some(
                deployment_status
                    .reconcile_status()
                    .map(|a| a.to_owned())
                    .unwrap_or_default(),
            ),
        );

        List::new(list_items)
            .block(normal_block(" DEPLOYMENT STATUS ".to_owned()))
            .render(body_sub_chunks[1], buf);
    }

    // Reserved
    {
        let mut list_items = Vec::<ListItem>::new();
        let device_reserved = app.mqtt_ctrl().device_reserved();
        let device_reserved_parsed = device_reserved.parse().unwrap_or_default();

        list_items_push(
            &mut list_items,
            "device",
            &Some(device_reserved_parsed.device.to_owned()),
        );

        list_items_push(
            &mut list_items,
            "version",
            &Some(device_reserved_parsed.dtmi_version.to_string()),
        );

        list_items_push(
            &mut list_items,
            "dtmi_path",
            &Some(device_reserved_parsed.dtmi_path.to_owned()),
        );
        List::new(list_items)
            .block(normal_block(" DEVICE RESERVED ".to_owned()))
            .render(body_sub_chunks[2], buf);
    }

    // Device States
    {
        let mut list_items = Vec::<ListItem>::new();

        let device_states = app.mqtt_ctrl().device_states();
        list_items_push(
            &mut list_items,
            "power_sources",
            &Some(device_states.power_state().power_sources()),
        );
        list_items_push(
            &mut list_items,
            "power_source_in_use",
            &Some(device_states.power_state().power_sources_in_use()),
        );
        list_items_push(
            &mut list_items,
            "is_battery_low",
            &Some(device_states.power_state().is_battery_low().to_string()),
        );
        list_items_push(
            &mut list_items,
            "process_state",
            &Some(device_states.process_state().to_owned()),
        );
        list_items_push(
            &mut list_items,
            "hours_meter",
            &Some(device_states.hours_meter().to_string()),
        );
        list_items_push(
            &mut list_items,
            "bootup_reason",
            &Some(device_states.bootup_reason()),
        );
        list_items_push(
            &mut list_items,
            "last_bootup_time",
            &Some(device_states.last_bootup_time().to_owned()),
        );
        List::new(list_items)
            .block(normal_block(" DEVICE STATE ".to_owned()))
            .render(body_sub_chunks[3], buf);
    }

    // Device Capabilities
    {
        let mut list_items = Vec::<ListItem>::new();

        let device_capabilities = app.mqtt_ctrl().device_capabilities();
        list_items_push(
            &mut list_items,
            "is_battery_supported",
            &Some(device_capabilities.is_battery_supported().to_string()),
        );
        list_items_push(
            &mut list_items,
            "supported_wireless_mode",
            &Some(device_capabilities.supported_wireless_mode()),
        );
        list_items_push(
            &mut list_items,
            "is_periodic_supported",
            &Some(device_capabilities.is_periodic_supported().to_string()),
        );
        list_items_push(
            &mut list_items,
            "is_sensor_postprocess_supported",
            &Some(
                device_capabilities
                    .is_sensor_postprocess_supported()
                    .to_string(),
            ),
        );
        List::new(list_items)
            .block(normal_block(" DEVICE CAPABILITIES ".to_owned()))
            .render(body_sub_chunks[4], buf);
    }

    let body_sub_chunks_right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .split(body_chunks[2]);

    //System Settings
    {
        let mut list_items = Vec::<ListItem>::new();
        let system_settings = app.mqtt_ctrl().system_settings();
        list_items_push(
            &mut list_items,
            "req_info.req_id",
            &Some(system_settings.req_info().req_id().to_owned()),
        );

        list_items_push(
            &mut list_items,
            "led_enabled",
            &Some(system_settings.led_enabled().to_string()),
        );

        list_items_push(
            &mut list_items,
            "temperature_update_interval",
            &Some(system_settings.temperature_update_interval().to_string()),
        );

        for l in system_settings.log_settings().iter() {
            let filter = l.filter();
            list_items_push(
                &mut list_items,
                &format!("log.{}.level", filter),
                &Some(l.level().to_string()),
            );
            list_items_push(
                &mut list_items,
                &format!("log.{}.destination", filter),
                &Some(l.destination().to_owned()),
            );
            list_items_push(
                &mut list_items,
                &format!("log.{}.storage_name", filter),
                &Some(l.storage_name().to_owned()),
            );
            list_items_push(
                &mut list_items,
                &format!("log{}.path", filter),
                &Some(l.path().to_owned()),
            );
        }

        list_items_push(
            &mut list_items,
            "res_info.res_id",
            &Some(system_settings.res_info().res_id().to_owned()),
        );

        list_items_push(
            &mut list_items,
            "res_info.code",
            &Some(system_settings.res_info().code_str().to_string()),
        );

        list_items_push(
            &mut list_items,
            "res_info.detail_msg",
            &Some(system_settings.res_info().detail_msg().to_owned()),
        );

        List::new(list_items)
            .block(normal_block(" SYSTEM SETTINGS ".to_owned()))
            .render(body_sub_chunks_right[0], buf);
    }

    // NetworkSettings
    {
        let mut list_items = Vec::<ListItem>::new();

        let network_settings = app.mqtt_ctrl().network_settings();

        list_items_push(
            &mut list_items,
            "req_info.req_id",
            &Some(network_settings.req_info().req_id().to_owned()),
        );

        let ip_method = network_settings.ip_method();
        let is_static = ip_method == "static".to_owned();
        list_items_push(&mut list_items, "ip_method", &Some(ip_method));
        list_items_push(
            &mut list_items,
            "ntp_url",
            &Some(network_settings.ntp_url()),
        );

        if is_static {
            if let Some(ipv4) = network_settings.ipv4() {
                list_items_push(&mut list_items, "ipv4_address", &Some(ipv4.ip_address()));

                list_items_push(
                    &mut list_items,
                    "ipv4_subnet_mask",
                    &Some(ipv4.subnet_mask()),
                );

                list_items_push(&mut list_items, "ipv4_gateway", &Some(ipv4.gateway()));

                list_items_push(&mut list_items, "ipv4_dns", &Some(ipv4.dns()));
            }

            if let Some(ipv6) = network_settings.ipv6() {
                list_items_push(&mut list_items, "ipv6_address", &Some(ipv6.ip_address()));

                list_items_push(
                    &mut list_items,
                    "ipv6_subnet_mask",
                    &Some(ipv6.subnet_mask()),
                );

                list_items_push(&mut list_items, "ipv6_gateway", &Some(ipv6.gateway()));

                list_items_push(&mut list_items, "ipv6_dns", &Some(ipv6.dns()));
            }
        }

        if let Some(proxy_settings) = network_settings.proxy() {
            list_items_push(
                &mut list_items,
                "proxy_url",
                &Some(proxy_settings.url().to_owned()),
            );
            list_items_push(
                &mut list_items,
                "proxy_port",
                &Some(proxy_settings.port().to_string()),
            );
            if let Some(user_name) = proxy_settings.user_name() {
                list_items_push(
                    &mut list_items,
                    "proxy_user_name",
                    &Some(user_name.to_owned()),
                );
            }
            if let Some(password) = proxy_settings.password() {
                list_items_push(
                    &mut list_items,
                    "proxy_password",
                    &Some(password.to_owned()),
                );
            }
        }

        list_items_push(
            &mut list_items,
            "res_info.res_id",
            &Some(network_settings.res_info().res_id().to_owned()),
        );

        list_items_push(
            &mut list_items,
            "res_info.code",
            &Some(network_settings.res_info().code_str().to_string()),
        );

        list_items_push(
            &mut list_items,
            "res_info.detail_msg",
            &Some(network_settings.res_info().detail_msg().to_owned()),
        );

        List::new(list_items)
            .block(normal_block(" NETWORK SETTINGS ".to_owned()))
            .render(body_sub_chunks_right[1], buf);
    }

    // Wireless Settings
    {
        let mut list_items = Vec::<ListItem>::new();
        let wireless_settings = app.mqtt_ctrl().wireless_settings();

        list_items_push(
            &mut list_items,
            "req_info.req_id",
            &Some(wireless_settings.req_info().req_id().to_owned()),
        );

        let station_setting = wireless_settings.sta_mode_setting();
        list_items_push(
            &mut list_items,
            "sta.ssid",
            &Some(station_setting.ssid().to_owned()),
        );

        list_items_push(
            &mut list_items,
            "sta.password",
            &Some(station_setting.password().to_owned()),
        );

        list_items_push(
            &mut list_items,
            "sta.encryption",
            &Some(station_setting.encryption().to_owned()),
        );

        list_items_push(
            &mut list_items,
            "res_info.res_id",
            &Some(wireless_settings.res_info().res_id().to_owned()),
        );

        list_items_push(
            &mut list_items,
            "res_info.code",
            &Some(wireless_settings.res_info().code_str().to_string()),
        );

        list_items_push(
            &mut list_items,
            "res_info.detail_msg",
            &Some(wireless_settings.res_info().detail_msg().to_owned()),
        );

        List::new(list_items)
            .block(normal_block(" WIRELESS SETTINGS ".to_owned()))
            .render(body_sub_chunks_right[2], buf);
    }
    //    // Main List
    //    let mut list_items = Vec::<ListItem>::new();
    //    for key in app.pairs.keys() {
    //        list_items.push(ListItem::new(Line::from(Span::styled(
    //            format!("{:<25}: {}", key, app.pairs.get(key).unwrap()),
    //            Style::default().fg(Color::Yellow),
    //        ))));
    //    }
    //    List::new(list_items)
    //        .block(Block::default().borders(Borders::ALL))
    //        .render(body_sub_chunks_right[2], buf);

    // Draw foot
    let foot_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[2]);

    let mut connect_info = Span::styled(" Disconnected ", Style::default().fg(Color::Red));

    let is_device_connected = app.mqtt_ctrl.is_device_connected();
    jdebug!(
        func = "render()",
        line = line!(),
        device_connected = format!("{:?}", is_device_connected)
    );

    let last_connected = app.mqtt_ctrl.last_connected_time();
    let now = Local::now();
    let delta = now - last_connected;
    let days = delta.num_days();
    let hours = delta.num_hours() % 24;
    let minutes = delta.num_minutes() % 60;
    let seconds = delta.num_seconds() % 60;

    let last_connected_str = format!(
        "{} ({} day {}h {}m {}s ago)",
        last_connected.format("%Y-%m-%d %H:%M:%S").to_string(),
        days,
        hours,
        minutes,
        seconds
    );

    let mut last_connected_info =
        Span::styled(&last_connected_str, Style::default().fg(Color::DarkGray));

    if is_device_connected {
        connect_info = Span::styled(" Connected ", Style::default().fg(Color::Green));
        last_connected_info = Span::styled(&last_connected_str, Style::default().fg(Color::White));
    }

    let current_navigation_text = vec![
        connect_info,
        Span::styled(" | ", Style::default().fg(Color::White)),
        last_connected_info,
    ];

    Paragraph::new(Line::from(current_navigation_text))
        .block(Block::default().borders(Borders::NONE))
        .render(foot_chunks[0], buf);

    let current_keys_hint = match app.current_screen {
        CurrentScreen::Main => Span::styled(
            "(q) to quit / (e) to make new pair",
            Style::default().fg(Color::White),
        ),

        CurrentScreen::Editing => Span::styled(
            "(ESC) to cancel / (Tab) to switch box/ Enter to complete",
            Style::default().fg(Color::White),
        ),
        CurrentScreen::Exiting => Span::styled(
            "(y) Exit and save status  / (n) Exit only / (c) Cancel",
            Style::default().fg(Color::White),
        ),
    };

    Paragraph::new(Line::from(current_keys_hint))
        .block(Block::default().borders(Borders::NONE))
        .render(foot_chunks[1], buf);
    Ok(())
}
