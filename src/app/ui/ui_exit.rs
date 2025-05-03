#[allow(unused)]
use {
    super::centered_rect,
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
pub fn draw(area: Rect, buf: &mut Buffer, _app: &App) -> Result<(), DMError> {
    let pop_area = centered_rect(80, 25, area);

    let popup_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Min(50),
            Constraint::Min(30),
            Constraint::Percentage(30),
        ])
        .split(pop_area);

    Paragraph::new("Do you want to save device status? (y/n/c)")
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .title(" EXIT ")
                .borders(Borders::ALL)
                .bg(Color::DarkGray),
        )
        .render(popup_chunks[1], buf);

    Ok(())
}
