#[allow(unused)]
use {
    super::centered_rect,
    super::*,
    crate::{
        app::{App, ConfigKey, DMScreen, MainWindowFocus},
        error::{DMError, DMErrorExt},
        mqtt_ctrl::MqttCtrl,
    },
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    error_stack::{Report, Result},
    jlogger_tracing::{JloggerBuilder, LevelFilter, LogTimeFormat, jdebug, jerror, jinfo},
    json::{JsonValue, object::Object},
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
    let elogs = app.mqtt_ctrl.elogs();

    let mut record = vec![];
    for elog in elogs.iter().rev() {
        let line = Line::from(vec![
            Span::styled(
                format!("{} ", elog.timestamp()),
                Style::default().fg(Color::White),
            )
            .into(),
            match elog.level() {
                0 => Span::styled(
                    format!("{:<8} ", elog.level_str()),
                    Style::default().fg(Color::Red),
                )
                .into(),
                1 => Span::styled(
                    format!("{:<8} ", elog.level_str()),
                    Style::default().fg(Color::Magenta),
                )
                .into(),
                2 => Span::styled(
                    format!("{:<8} ", elog.level_str()),
                    Style::default().fg(Color::Yellow),
                )
                .into(),
                _ => Span::styled(
                    format!("{:<8} ", elog.level_str()),
                    Style::default().fg(Color::White),
                )
                .into(),
            },
            Span::styled(
                format!("{} (0x{:0x})", elog.event_str(), elog.event_id()),
                Style::default().fg(Color::White),
            )
            .into(),
            Span::styled("\n", Style::default().fg(Color::White)).into(),
        ]);
        record.push(line);
    }

    if !record.is_empty() {
        Paragraph::new(record)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" ELOGS ")
                    .border_style(Style::default().fg(Color::White)),
            )
            .render(area, buf);
    }

    Ok(())
}
