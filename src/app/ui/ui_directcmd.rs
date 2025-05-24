#[allow(unused)]
use {
    super::centered_rect,
    super::*,
    crate::{
        app::{App, ConfigKey, DMScreen, DirectCommand, MainWindowFocus},
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

pub fn draw_reboot(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Draw request
    {
        let message = match &app.mqtt_ctrl.direct_command_request() {
            Some(Ok(m)) => {
                if let Ok(j) = json::parse(m) {
                    let mut root = Object::new();
                    root.insert("command", JsonValue::String("reboot".to_owned()));
                    root.insert("request", j);

                    json::stringify_pretty(root, 4)
                } else {
                    m.to_owned()
                }
            }
            Some(Err(e)) => e
                .error_str()
                .unwrap_or_else(|| "Failed to send reboot direct command".to_string()),
            None => "Sending reboot command...".to_string(),
        };

        let paragraph = Paragraph::new(message)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Direct Command Request "),
            )
            .alignment(Alignment::Left);
        paragraph.render(chunks[0], buf);
    }

    // Draw response
    {
        let message = match &app.mqtt_ctrl.direct_command_result() {
            Some(Ok(m)) => {
                if let Ok(j) = json::parse(m) {
                    let execute_time = app.mqtt_ctrl.direct_command_exec_time().unwrap();

                    let mut root = Object::new();
                    root.insert("command", JsonValue::String("reboot".to_owned()));
                    root.insert("response", j);
                    root.insert("execute_time_ms", JsonValue::Number(execute_time.into()));

                    json::stringify_pretty(root, 4)
                } else {
                    m.to_owned()
                }
            }
            Some(Err(e)) => e
                .error_str()
                .unwrap_or_else(|| "Failed to receive reboot direct command response".to_string()),
            None => "Waiting for reboot response...".to_string(),
        };

        let paragraph = Paragraph::new(message)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Direct Command Response "),
            )
            .alignment(Alignment::Left);
        paragraph.render(chunks[1], buf);
    }
    {}

    Ok(())
}

pub fn draw_get_direct_image(area: Rect, buf: &mut Buffer, _app: &App) -> Result<(), DMError> {
    let paragraph = Paragraph::new("Retrieving preview image...")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Direct Command"),
        )
        .alignment(Alignment::Left);
    paragraph.render(area, buf);
    Ok(())
}

pub fn draw_factory_reset(area: Rect, buf: &mut Buffer, _app: &App) -> Result<(), DMError> {
    let paragraph = Paragraph::new("Executing factory reset...")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Direct Command"),
        )
        .alignment(Alignment::Left);
    paragraph.render(area, buf);
    Ok(())
}

pub fn draw(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    match app.mqtt_ctrl.get_direct_command() {
        Some(DirectCommand::Reboot) => draw_reboot(area, buf, app)?,
        Some(DirectCommand::GetDirectImage) => draw_get_direct_image(area, buf, app)?,
        Some(DirectCommand::FactoryReset) => draw_factory_reset(area, buf, app)?,
        None => {
            let message = r#"
What direct command do you want to send?

You can use the following commands:

- Press 'r' to reboot the device.
- Press 'i' to retrieve preview image (GetDirectImage).
- Press 'f' to execute Factory Reset.

Press 'Esc' to return to the main menu.
"#;
            let paragraph = Paragraph::new(message)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Direct Command"),
                )
                .alignment(Alignment::Left);
            paragraph.render(area, buf);
        }
        _ => {
            let paragraph = Paragraph::new("Unsupported command")
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Direct Command"),
                )
                .alignment(Alignment::Left);
            paragraph.render(area, buf);
        }
    }

    Ok(())
}
