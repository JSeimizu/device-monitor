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
    if let Some(result) = app.config_result.as_ref() {
        let popup_chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([Constraint::Percentage(100)])
            .split(area);
        match result {
            Ok(s) => {
                let block = normal_block("Configuration Result");
                let root = json::parse(s).unwrap();

                for (k, v) in root.entries() {
                    // Json entry in DTDL for SystemApp is stored as json string
                    // transfer it to normal json object for a pretty view.
                    if let JsonValue::String(s) = v {
                        let json = json::parse(s).unwrap();
                        let mut root = Object::new();
                        root.insert(k, json);
                        Paragraph::new(json::stringify_pretty(root, 4))
                            .block(block)
                            .render(popup_chunks[0], buf);
                        break;
                    } else {
                        Paragraph::new(s.to_owned())
                            .block(block)
                            .render(popup_chunks[0], buf);
                        break;
                    }
                }
            }
            Err(e) => {
                let block = normal_block("Configuration Error");
                let s = e.error_str().unwrap();
                Paragraph::new(s).block(block).render(popup_chunks[0], buf);
            }
        }
        Ok(())
    } else {
        let popup_chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        let focus = app.main_window_focus();
        let mut block = normal_block(" Configuration ");
        let mut sample = String::new();
        let mut note = String::new();
        match focus {
            MainWindowFocus::DeploymentStatus => {
                block = normal_block(" Configuration for EdgeApp Deployment ");
            }
            MainWindowFocus::SystemSettings => {
                block = normal_block(" Configuration for System Settings ");
                sample = include_str!("../../../sample/system_settings.json").to_owned();
            }
            MainWindowFocus::NetworkSettings => {
                block = normal_block(" Configuration for Network Settings ");
                sample = include_str!("../../../sample/network_settings.json").to_owned();
            }
            MainWindowFocus::WirelessSettings => {
                block = normal_block(" Configuration for Wireless Settings ");
                sample = include_str!("../../../sample/wireless_settings.json").to_owned();
            }

            MainWindowFocus::MainChip
            | MainWindowFocus::SensorChip
            | MainWindowFocus::CompanionChip => {
                block = normal_block(" Configuration for OTA");
                note.push_str("\n\n");
                note.push_str("  ota_fw.json is used for firmware OTA\n");
                note.push_str("  ota_ai_model.json is used for AI Model OTA\n");
            }
            _ => {}
        };

        let message = format!(
            "\n  Please describe configuration in following json file:\n\n    {}/{}",
            App::config_dir(),
            focus.user_config_file()
        );

        Paragraph::new(message)
            .block(block)
            .render(popup_chunks[0], buf);

        if !sample.is_empty() {
            let block = normal_block(" Sample ");
            let json = json::parse(&sample).unwrap();
            Paragraph::new(json::stringify_pretty(json, 4))
                .block(block)
                .render(popup_chunks[1], buf);
        }

        if !note.is_empty() {
            let block = normal_block(" Note ");
            Paragraph::new(note)
                .block(block)
                .render(popup_chunks[1], buf);
        }
        Ok(())
    }
}
