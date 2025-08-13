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

use crate::{app::App, error::DMError, ota::{ProcessState, Target}};

#[allow(unused)]
use {
    ratatui::{
        buffer::Buffer,
        layout::{Alignment, Constraint, Direction, Layout, Rect},
        prelude::{Color, Style},
        style::Stylize,
        symbols::border,
        text::{Line, Span, Text},
        widgets::{Block, Borders, List, ListItem, Paragraph, Widget},
    },
};

pub fn draw(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    let firmware = app.firmware();
    
    let main_block = Block::default()
        .title("OTA Firmware Update")
        .borders(Borders::ALL)
        .border_set(border::ROUNDED);
    
    let inner_area = main_block.inner(area);
    main_block.render(area, buf);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(inner_area);

    draw_chip_section(chunks[0], buf, "Main Chip", &firmware.main_chip)?;
    draw_chip_section(chunks[1], buf, "Companion Chip", &firmware.companion_chip)?;
    draw_chip_section(chunks[2], buf, "Sensor Chip", &firmware.sensor_chip)?;

    Ok(())
}

fn draw_chip_section(area: Rect, buf: &mut Buffer, title: &str, target: &Target) -> Result<(), DMError> {
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_set(border::ROUNDED);
    
    let inner_area = block.inner(area);
    block.render(area, buf);

    let items = vec![
        format!("Component: {:?}", target.component),
        format!("Chip: {}", target.chip),
        format!("Version: {}", if target.version.is_empty() { "N/A" } else { &target.version }),
        format!("Progress: {}%", target.progress),
        format!("State: {}", format_process_state(&target.process_state)),
        format!("Package URL: {}", if target.package_url.is_empty() { "N/A" } else { &target.package_url }),
        format!("Hash: {}", if target.hash.is_empty() { "N/A" } else { &target.hash }),
        format!("Size: {} bytes", target.size),
    ];

    let list_items: Vec<ListItem> = items
        .iter()
        .map(|item| {
            ListItem::new(Line::from(Span::raw(item.as_str())))
        })
        .collect();

    let list = List::new(list_items)
        .style(Style::default());

    list.render(inner_area, buf);

    Ok(())
}

fn format_process_state(state: &ProcessState) -> &'static str {
    match state {
        ProcessState::Idle => "Idle",
        ProcessState::RequestReceived => "Request Received",
        ProcessState::Downloading => "Downloading",
        ProcessState::Installing => "Installing",
        ProcessState::Done => "Done",
        ProcessState::Failed => "Failed",
        ProcessState::FailedInvalidArgument => "Failed (Invalid Argument)",
        ProcessState::FailedTokenExpired => "Failed (Token Expired)",
        ProcessState::FailedDownloadRetryExceeded => "Failed (Download Retry Exceeded)",
    }
}