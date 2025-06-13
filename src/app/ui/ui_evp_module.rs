use crate::azurite::AzuriteAction;
#[allow(unused)]
use {
    super::*,
    crate::{
        app::{App, AzuriteStorage, DMScreen},
        error::DMError,
        mqtt_ctrl::{
            MqttCtrl,
            evp::device_info::{ChipInfo, DeviceInfo},
            evp::evp_state::{AgentDeviceConfig, AgentSystemInfo, UUID},
            evp::{
                device_info::{
                    DeviceCapabilities, DeviceReserved, DeviceStates, NetworkSettings,
                    SystemSettings, WirelessSettings,
                },
                evp_state::DeploymentStatus,
            },
        },
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

fn do_list_modules(
    azure_storage: &AzuriteStorage,
    area: Rect,
    buf: &mut Buffer,
) -> Result<(), DMError> {
    let mut list_items = Vec::<ListItem>::new();
    let module_info_db = azure_storage.module_info_db();
    let mut no = 1;
    for (id, (uuid, module_info)) in module_info_db.iter().enumerate() {
        let text = format!(
            "No: {}  ModuleID: {}  ContainerName: {}  BlobName: {}",
            no,
            uuid.uuid(),
            module_info.container_name,
            module_info.blob_name
        );

        list_items_push_text_focus(
            &mut list_items,
            &text,
            id == azure_storage.current_module_id(),
        );
        no += 1;
    }

    let title = " Azurite Storage Modules ";
    let block = normal_block(title);

    List::new(list_items).block(block).render(area, buf);
    Ok(())
}

fn do_add(azure_storage: &AzuriteStorage, area: Rect, buf: &mut Buffer) -> Result<(), DMError> {
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

    Paragraph::new(format!("{}|", azure_storage.new_module()))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .title(" Input New Module File Path ")
                .borders(Borders::ALL)
                .bg(Color::DarkGray),
        )
        .render(popup_chunks[1], buf);

    Ok(())
}

pub fn draw(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    if let Some(azure_storage) = &app.azurite_storage {
        match azure_storage.action() {
            AzuriteAction::Deploy => do_list_modules(azure_storage, area, buf)?,
            AzuriteAction::Add => do_add(azure_storage, area, buf)?,
        }
    }

    Ok(())
}
