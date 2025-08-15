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

use crate::{azurite::AzuriteAction, error::DMErrorExt};
#[allow(unused)]
use {
    super::*,
    crate::{
        app::{App, DMScreen},
        azurite::{AzuriteStorage, with_azurite_storage},
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

pub fn do_deploy(
    area: Rect,
    buf: &mut Buffer,
    config_result: &Result<String, DMError>,
) -> Result<(), DMError> {
    let message = match config_result {
        Ok(config) => config.clone(),
        Err(e) => e.error_str().unwrap_or("Unknown error".to_owned()),
    };

    let paragraph = Paragraph::new(message)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Deploy EdgeApp Request "),
        )
        .alignment(Alignment::Left);
    paragraph.render(area, buf);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    fn test_do_deploy_ok_and_err() {
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);

        // Successful config result
        let ok_res: error_stack::Result<String, crate::error::DMError> =
            Ok("config-ok".to_string());
        assert!(do_deploy(area, &mut buf, &ok_res).is_ok());

        // Error config result (ensure do_deploy handles error variant without panic)
        let err_res: error_stack::Result<String, crate::error::DMError> =
            Err(error_stack::Report::new(crate::error::DMError::InvalidData));
        assert!(do_deploy(area, &mut buf, &err_res).is_ok());
    }
}

fn do_list_modules(
    azure_storage: &AzuriteStorage,
    area: Rect,
    buf: &mut Buffer,
) -> Result<(), DMError> {
    let mut list_items = Vec::<ListItem>::new();
    let module_info_db = azure_storage.module_info_db();
    let mut no = 1;
    for (id, (uuid, module_info)) in module_info_db.iter().enumerate() {
        let focus = id == azure_storage.current_module_id();
        let text = format!(
            "No{:2}  ModuleID: {}  ContainerName: {}  BlobName: {}",
            no,
            uuid.uuid(),
            module_info.container_name,
            module_info.blob_name,
        );
        list_items_push_text_focus(&mut list_items, &text, focus);

        let text = format!("      Hash: {}", module_info.hash,);
        list_items_push_text_focus(&mut list_items, &text, focus);

        let text = format!("      URL: {}", module_info.sas_url,);
        list_items_push_text_focus(&mut list_items, &text, focus);

        let text = format!("      Size: {}", module_info.size,);
        list_items_push_text_focus(&mut list_items, &text, focus);

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
    if let Some(action) = with_azurite_storage(|azure_storage| azure_storage.action()) {
        match action {
            Some(AzuriteAction::Deploy) => {
                if let Some(config_result) = &app.config_result {
                    do_deploy(area, buf, config_result)?;
                } else {
                    with_azurite_storage(|azure_storage| do_list_modules(azure_storage, area, buf))
                        .unwrap_or(Ok(()))?;
                }
            }
            Some(AzuriteAction::Select) => {
                with_azurite_storage(|azure_storage| do_list_modules(azure_storage, area, buf))
                    .unwrap_or(Ok(()))?;
            }
            Some(AzuriteAction::Add) => {
                with_azurite_storage(|azure_storage| do_add(azure_storage, area, buf))
                    .unwrap_or(Ok(()))?;
            }

            _ => {}
        }
    }

    Ok(())
}
