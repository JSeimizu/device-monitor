#[allow(unused)]
use {
    super::*,
    crate::{
        app::{App, AzuriteStorage, DMScreen},
        azurite::TokenProvider,
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

        let text = format!("      SAS URL: {}", token_provider.sas_url);
        list_items_push_text_focus(&mut list_items, &text, focus);
        no += 1;
    }

    let title = " Token Providers ";
    let block = normal_block(title);

    List::new(list_items).block(block).render(area, buf);
    Ok(())
}

pub fn draw(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    if let Some(azure_storage) = &app.azurite_storage {
        do_list_token_providers(azure_storage, area, buf)?;
    }

    Ok(())
}
