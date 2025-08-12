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

#[allow(unused)]
use {
    super::centered_rect,
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
pub fn draw(area: Rect, buf: &mut Buffer, _app: &App) -> Result<(), DMError> {
    let pop_area = centered_rect(80, 25, area);

    let popup_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Min(50),
            Constraint::Min(30),
            Constraint::Percentage(30),
        ])
        .split(pop_area);

    Paragraph::new("Do you want to exit? (y/n)")
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    fn test_draw_returns_ok() {
        // Construct an App via the public constructor to pass into the draw function.
        let app = crate::app::App::new(crate::app::AppConfig { broker: "b" }).unwrap();

        // Prepare drawing area and buffer
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);

        // draw should succeed (return Ok)
        assert!(draw(area, &mut buf, &app).is_ok());
    }
}
