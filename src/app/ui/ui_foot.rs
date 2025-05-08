#[allow(unused)]
use {
    crate::{
        app::{App, CurrentScreen, CurrentlyEditing},
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
    // Draw foot
    let foot_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

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
            "(q) to quit, (Enter) full-screen",
            Style::default().fg(Color::White),
        ),
        CurrentScreen::CompanionChip
        | CurrentScreen::MainChip
        | CurrentScreen::AgentState
        | CurrentScreen::DeviceReserved
        | CurrentScreen::DeviceCapabilities
        | CurrentScreen::DeviceState
        | CurrentScreen::DeploymentStatus
        | CurrentScreen::SystemSettings
        | CurrentScreen::NetworkSettings
        | CurrentScreen::WirelessSettings => Span::styled(
            "(Enter)/(Esc) back to main screen, (q) to quit",
            Style::default().fg(Color::White),
        ),

        CurrentScreen::Editing => Span::styled(
            "(ESC) to cancel / (Tab) to switch box/ Enter to complete",
            Style::default().fg(Color::White),
        ),
        CurrentScreen::Exiting => {
            Span::styled("(y) Exit / (n) Cancel", Style::default().fg(Color::White))
        }
    };

    Paragraph::new(Line::from(current_keys_hint))
        .block(Block::default().borders(Borders::NONE))
        .render(foot_chunks[1], buf);

    Ok(())
}
