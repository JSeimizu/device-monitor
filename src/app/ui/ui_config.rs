use crate::app::ConfigKey;
#[allow(unused)]
use {
    super::centered_rect,
    super::*,
    crate::{
        app::{App, DMScreen},
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
    let focus = |config_key| ConfigKey::from(app.config_key_focus) == config_key;

    let value = |config_key| {
        if app.config_key_editable && focus(config_key) {
            format!("{}|", &app.config_keys[usize::from(config_key)])
        } else {
            format!("{}", &app.config_keys[usize::from(config_key)])
        }
    };

    let mut list_items = Vec::<ListItem>::new();
    list_items_push_focus(
        &mut list_items,
        "report_status_interval_min",
        &value(ConfigKey::ReportStatusIntervalMin),
        focus(ConfigKey::ReportStatusIntervalMin),
    );

    list_items_push_focus(
        &mut list_items,
        "report_status_interval_max",
        &value(ConfigKey::ReportStatusIntervalMax),
        focus(ConfigKey::ReportStatusIntervalMax),
    );

    List::new(list_items)
        .block(normal_block(" Configuration "))
        .render(area, buf);

    Ok(())
}
