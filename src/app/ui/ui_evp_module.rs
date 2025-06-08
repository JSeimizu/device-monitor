#[allow(unused)]
use {
    super::*,
    crate::{
        app::{App, DMScreen},
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

pub fn draw(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    let mut list_items = Vec::<ListItem>::new();
    if let Some(azure_storage) = &app.azurite_storage {
        let module_info_db = azure_storage.module_info_db();
        let mut no = 1;
        for (uuid, module_info) in module_info_db {
            let text = Text::from(Span::raw(format!(
                "No: {}  ModuleID: {}  ContainerName: {}  BlobName: {}",
                no,
                uuid.uuid(),
                module_info.container_name,
                module_info.blob_name
            )));
            list_items.push(ListItem::new(text));
            no += 1;
        }
    }

    let title = " Azurite Storage Modules ";
    let block = normal_block(title);

    List::new(list_items).block(block).render(area, buf);

    Ok(())
}
