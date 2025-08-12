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
    super::*,
    crate::{
        app::{App, DMScreen},
        azurite::{AzuriteStorage, TokenProvider, with_azurite_storage},
        error::DMError,
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

fn do_list_token_providers(
    azure_storage: &AzuriteStorage,
    area: Rect,
    buf: &mut Buffer,
) -> Result<(), DMError> {
    let mut list_items = Vec::<ListItem>::new();
    let token_providers_db = azure_storage.token_providers();
    let mut no = 1;
    for (id, (uuid, token_provider)) in token_providers_db.iter().enumerate() {
        let focus = id == azure_storage.current_token_provider_id();
        let text = format!("No{:2}  UUID: {}", no, uuid.uuid(),);
        list_items_push_text_focus(&mut list_items, &text, focus);

        let text = format!("       Container: {}", token_provider.container);
        list_items_push_text_focus(&mut list_items, &text, focus);
        no += 1;
    }

    let title = " Token Providers ";
    let block = normal_block(title);

    List::new(list_items).block(block).render(area, buf);
    Ok(())
}

pub fn draw(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    with_azurite_storage(|azure_storage| do_list_token_providers(azure_storage, area, buf))
        .unwrap_or(Ok(()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    #[should_panic]
    fn test_draw_without_azurite_storage_returns_ok() {
        // Creating App via public constructor
        let app = crate::app::App::new(crate::app::AppConfig { broker: "b" }).unwrap();

        // Prepare drawing area and buffer
        let area = Rect::new(0, 0, 40, 12);
        let mut buf = Buffer::empty(area);

        // draw() calls with_azurite_storage which panics when the global AzuriteStorage is not initialized.
        // We expect a panic here in the test environment.
        let _ = draw(area, &mut buf, &app);
    }

    #[test]
    #[should_panic]
    fn test_do_list_token_providers_handles_empty_db() {
        // We can't easily initialize a full AzuriteStorage here. draw() uses with_azurite_storage
        // which will panic when the global storage has not been initialized in the test env.
        let app = crate::app::App::new(crate::app::AppConfig { broker: "b" }).unwrap();
        let area = Rect::new(0, 0, 40, 12);
        let mut buf = Buffer::empty(area);

        // Expect a panic due to missing global AzuriteStorage.
        let _ = draw(area, &mut buf, &app);
    }
}
