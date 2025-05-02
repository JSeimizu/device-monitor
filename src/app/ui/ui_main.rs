#[allow(unused)]
use {
    crate::{
        app::{App, CurrentScreen, CurrentlyEditing},
        error::DMError,
        mqtt_ctrl::MqttCtrl,
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
    let mut list_items = Vec::<ListItem>::new();
    for key in app.pairs.keys() {
        list_items.push(ListItem::new(Line::from(Span::styled(
            format!("{:<25}: {}", key, app.pairs.get(key).unwrap()),
            Style::default().fg(Color::Yellow),
        ))));
    }
    List::new(list_items)
        .block(Block::default().borders(Borders::NONE))
        .render(chunks[1], buf);

    // Draw foot
    let foot_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(chunks[2]);

    let mut connect_info = Span::styled(" Disconnected ", Style::default().fg(Color::Red));

    let is_device_connected = app.mqtt_ctrl.is_device_connected();
    jdebug!(
        func = "render()",
        line = line!(),
        device_connected = format!("{:?}", is_device_connected)
    );

    if is_device_connected {
        connect_info = Span::styled(" Connected ", Style::default().fg(Color::Green));
    }

    let current_navigation_text = vec![
        connect_info,
        Span::styled(" | ", Style::default().fg(Color::White)),
        match app.currently_editing {
            CurrentlyEditing::Key => {
                Span::styled("Editing Json Key", Style::default().fg(Color::Green))
            }
            CurrentlyEditing::Value => {
                Span::styled("Editing Json Value", Style::default().fg(Color::LightGreen))
            }
            CurrentlyEditing::None => {
                Span::styled("Not Editing", Style::default().fg(Color::DarkGray))
            }
        },
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
