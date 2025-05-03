use std::cmp::min;

use chrono::Local;
#[allow(unused)]
use {
    crate::{
        app::{App, CurrentScreen, CurrentlyEditing},
        error::DMError,
        mqtt_ctrl::{
            MqttCtrl,
            evp::device_info::{ChipInfo, DeviceInfo},
            evp::evp_state::{AgentState, SystemInfo},
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
    let mut list_items_push =
        |list_items: &mut Vec<ListItem>, name: &str, value: &Option<String>| {
            list_items.push(ListItem::new(Span::styled(
                format!("{:<25} : {}", name, value.as_deref().unwrap_or("-")),
                Style::default(),
            )));
        };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(area);

    // Draw title
    Paragraph::new(Text::styled(
        "Device Monitor",
        Style::default().fg(Color::White),
    ))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::empty()))
    .render(chunks[0], buf);

    // Draw body
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    // Device Info
    {
        let device_info_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Min(3),
            ])
            .split(body_chunks[0]);

        // Device manifest
        {
            let device_manifest = app
                .mqtt_ctrl()
                .device_info()
                .map(|d| d.device_manifest().unwrap_or("-"))
                .unwrap_or("-");

            Paragraph::new(device_manifest)
                .block(
                    Block::default()
                        .title(" device manifest ")
                        .borders(Borders::ALL),
                )
                .render(device_info_chunks[3], buf);
        }

        let mut create_list = |chip_name: &str| {
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

            if let Some(device_info) = app.mqtt_ctrl().device_info() {
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

            let title = format!(" {} ", chip_name.replace("_", " "));
            List::new(list_items)
                .block(Block::default().title(title).borders(Borders::ALL))
                .render(device_info_chunks[dev_info_chunk_index], buf);
        };

        // main_chip
        create_list("main_chip");
        // companion_chip
        create_list("companion_chip");
        //sensor_chip
        create_list("sensor_chip");
    }

    let body_sub_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(60),
        ])
        .split(body_chunks[1]);

    // Agent State
    {
        let mut list_items = Vec::<ListItem>::new();
        let mut agent_state = AgentState::default();
        let mut r_agent_state = &agent_state;

        if let Some(s) = app.mqtt_ctrl().agent_state() {
            r_agent_state = s;
        }
        list_items_push(&mut list_items, "os", &r_agent_state.system_info.os);
        list_items_push(&mut list_items, "arch", &r_agent_state.system_info.arch);
        list_items_push(
            &mut list_items,
            "evp_agent",
            &r_agent_state.system_info.evp_agent,
        );
        list_items_push(
            &mut list_items,
            "evp_agent_commit_hash",
            &r_agent_state.system_info.evp_agent_commit_hash,
        );
        list_items_push(
            &mut list_items,
            "wasmMicroRuntime",
            &r_agent_state.system_info.wasmMicroRuntime,
        );
        list_items_push(
            &mut list_items,
            "protocolVersion",
            &r_agent_state.system_info.protocolVersion,
        );
        list_items_push(
            &mut list_items,
            "report-status-interval-min",
            &Some(r_agent_state.report_status_interval_min.to_string()),
        );
        list_items_push(
            &mut list_items,
            "report-status-interval-max",
            &Some(r_agent_state.report_status_interval_max.to_string()),
        );
        list_items_push(
            &mut list_items,
            "deploymentStatus",
            &r_agent_state.system_info.deploymentStatus,
        );

        List::new(list_items)
            .block(
                Block::default()
                    .title(" agent state ")
                    .borders(Borders::ALL),
            )
            .render(body_sub_chunks[0], buf);
    }

    // Main List
    let mut list_items = Vec::<ListItem>::new();
    for key in app.pairs.keys() {
        list_items.push(ListItem::new(Line::from(Span::styled(
            format!("{:<25}: {}", key, app.pairs.get(key).unwrap()),
            Style::default().fg(Color::Yellow),
        ))));
    }
    List::new(list_items)
        .block(Block::default().borders(Borders::ALL))
        .render(body_sub_chunks[1], buf);

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
