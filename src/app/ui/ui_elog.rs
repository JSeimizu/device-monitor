/*
Copyright [2025] Seimizu Joukan

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

use crate::mqtt_ctrl::with_mqtt_ctrl;
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

pub fn draw(area: Rect, buf: &mut Buffer, _app: &App) -> Result<(), DMError> {
    with_mqtt_ctrl(|mqtt_ctrl| -> Result<(), DMError> {
        let elogs = mqtt_ctrl.elogs();

        let mut record = vec![];
        for elog in elogs.iter().rev() {
            let line = Line::from(vec![
                Span::styled(
                    format!("{} ", elog.timestamp()),
                    Style::default().fg(Color::White),
                ),
                match elog.level() {
                    0 => Span::styled(
                        format!("{:<8} ", elog.level_str()),
                        Style::default().fg(Color::Red),
                    ),
                    1 => Span::styled(
                        format!("{:<8} ", elog.level_str()),
                        Style::default().fg(Color::Magenta),
                    ),
                    2 => Span::styled(
                        format!("{:<8} ", elog.level_str()),
                        Style::default().fg(Color::Yellow),
                    ),
                    _ => Span::styled(
                        format!("{:<8} ", elog.level_str()),
                        Style::default().fg(Color::White),
                    ),
                },
                Span::styled(
                    format!("{} (0x{:0x})", elog.event_str(), elog.event_id()),
                    Style::default().fg(Color::White),
                ),
                Span::styled("\n", Style::default().fg(Color::White)),
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    #[should_panic]
    fn test_draw_panics_when_mqtt_uninitialized() {
        // Building App via public constructor
        let app = crate::app::App::new(crate::app::AppConfig { broker: "b" }).unwrap();

        // Prepare drawing area and buffer
        let area = Rect::new(0, 0, 40, 12);
        let mut buf = Buffer::empty(area);

        // draw() calls with_mqtt_ctrl which will panic when the global MqttCtrl is not initialized.
        let _ = draw(area, &mut buf, &app);
    }
}
