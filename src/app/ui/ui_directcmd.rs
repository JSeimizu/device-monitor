use crate::mqtt_ctrl::with_mqtt_ctrl;
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

pub fn draw_reboot(
    area: Rect,
    buf: &mut Buffer,
    app: &App,
    mqtt_ctrl: &MqttCtrl,
) -> Result<(), DMError> {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Draw request
    {
        let message = match mqtt_ctrl.direct_command_request() {
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
        let message = match mqtt_ctrl.direct_command_result() {
            Some(Ok(m)) => {
                let execute_time = mqtt_ctrl.direct_command_exec_time().unwrap();
                let s = m.to_string();
                let j = json::parse(&s).unwrap_or(JsonValue::Null);

                let mut root = Object::new();
                root.insert("command", JsonValue::String("reboot".to_owned()));
                root.insert("response", j);
                root.insert("execute_time_ms", JsonValue::Number(execute_time.into()));

                json::stringify_pretty(root, 4)
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

pub fn draw_get_direct_image(
    area: Rect,
    buf: &mut Buffer,
    app: &App,
    mqtt_ctrl: &MqttCtrl,
) -> Result<(), DMError> {
    if let Some(result) = mqtt_ctrl.direct_command_request() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);

        // Draw request
        {
            let message = match result {
                Ok(m) => {
                    if let Ok(j) = json::parse(m) {
                        let mut root = Object::new();
                        root.insert("command", JsonValue::String("direct_get_image".to_owned()));
                        root.insert("request", j);

                        json::stringify_pretty(root, 4)
                    } else {
                        m.to_owned()
                    }
                }
                Err(e) => e.error_str().unwrap_or_else(|| {
                    "Failed to send direct_get_image direct command".to_string()
                }),
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
            let message = match mqtt_ctrl.direct_command_result() {
                Some(Ok(m)) => {
                    let execute_time = mqtt_ctrl.direct_command_exec_time().unwrap();

                    if let Ok(response) = serde_json::to_string(&m) {
                        let response = json::parse(&response).unwrap_or(JsonValue::Null);

                        let mut root = Object::new();
                        root.insert("command", JsonValue::String("direct_get_image".to_owned()));
                        root.insert("response", response);
                        root.insert("execute_time_ms", JsonValue::Number(execute_time.into()));
                        json::stringify_pretty(root, 4)
                    } else {
                        m.to_string()
                    }
                }
                Some(Err(e)) => e.error_str().unwrap_or_else(|| {
                    "Failed to receive reboot direct command response".to_string()
                }),
                None => "Waiting for direct_get_image response...".to_string(),
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
    } else {
        // GetDirectImage configuration UI.
        let focus = |config_key| ConfigKey::from(app.config_key_focus) == config_key;

        let value = |config_key| {
            let value = app
                .config_keys
                .get(usize::from(config_key))
                .map(|s| s.as_str())
                .unwrap_or_default();

            if app.config_key_editable && focus(config_key) {
                format!("{}|", value)
            } else {
                value.to_string()
            }
        };

        let mut list_items = Vec::<ListItem>::new();
        list_items_push_focus(
            &mut list_items,
            "sensor_name",
            &value(ConfigKey::DirectGetImageSensorName),
            focus(ConfigKey::DirectGetImageSensorName),
        );

        list_items_push_focus(
            &mut list_items,
            "network_id",
            &value(ConfigKey::DirectGetImageNetworkId),
            focus(ConfigKey::DirectGetImageNetworkId),
        );
        List::new(list_items)
            .block(normal_block(" Configuration for GetDirectImage "))
            .render(area, buf);
    }

    Ok(())
}

pub fn draw_factory_reset(
    area: Rect,
    buf: &mut Buffer,
    _app: &App,
    mqtt_ctrl: &MqttCtrl,
) -> Result<(), DMError> {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Draw request
    {
        let message = match mqtt_ctrl.direct_command_request() {
            Some(Ok(m)) => {
                if let Ok(j) = json::parse(m) {
                    let mut root = Object::new();
                    root.insert("command", JsonValue::String("factory_reset".to_owned()));
                    root.insert("request", j);

                    json::stringify_pretty(root, 4)
                } else {
                    m.to_owned()
                }
            }
            Some(Err(e)) => e
                .error_str()
                .unwrap_or_else(|| "Failed to send factory_reset direct command".to_string()),
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
        let message = match mqtt_ctrl.direct_command_result() {
            Some(Ok(m)) => {
                let execute_time = mqtt_ctrl.direct_command_exec_time().unwrap();
                let s = m.to_string();
                let j = json::parse(&s).unwrap_or(JsonValue::Null);

                let mut root = Object::new();
                root.insert("command", JsonValue::String("factory_reset".to_owned()));
                root.insert("response", j);
                root.insert("execute_time_ms", JsonValue::Number(execute_time.into()));

                json::stringify_pretty(root, 4)
            }
            Some(Err(e)) => e.error_str().unwrap_or_else(|| {
                "Failed to receive factory_reset direct command response".to_string()
            }),
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

    Ok(())
}

pub fn draw(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    with_mqtt_ctrl(|mqtt_ctrl| -> Result<(), DMError> {
        match mqtt_ctrl.get_direct_command() {
            Some(DirectCommand::Reboot) => draw_reboot(area, buf, app, mqtt_ctrl)?,
            Some(DirectCommand::GetDirectImage) => {
                draw_get_direct_image(area, buf, app, mqtt_ctrl)?
            }
            Some(DirectCommand::FactoryReset) => draw_factory_reset(area, buf, app, mqtt_ctrl)?,
            None => {
                let message = r#"
 What direct command do you want to send?

 You can use the following commands:

   - Press 'r' to reboot the device.
   - Press 'i' to retrieve preview image (DirectGetImage).
   - Press 'f' to execute Factory Reset.

 Press 'Esc' to return to the main menu.
"#;
                let paragraph = Paragraph::new(message)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" Direct Command "),
                    )
                    .alignment(Alignment::Left);
                paragraph.render(area, buf);
            }
            _ => {
                let paragraph = Paragraph::new("Unsupported command")
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" Direct Command "),
                    )
                    .alignment(Alignment::Left);
                paragraph.render(area, buf);
            }
        }
        Ok(())
    })
}
