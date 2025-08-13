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

use crate::{
    app::App,
    error::DMError,
    ota::{Component, FirmwareProperty, ProcessState, Target},
};

#[allow(unused)]
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    prelude::{Color, Style},
    style::Stylize,
    symbols::border,
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Widget},
};

pub fn draw(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    let firmware = app.firmware();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(6),    // ReqInfo/ResInfo section
            Constraint::Min(0),       // Chip sections
        ])
        .split(area);

    // Draw req_info and res_info section
    draw_info_section(chunks[0], buf, firmware)?;

    // Draw chip sections
    let chip_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(chunks[1]);

    let chips = ["main_chip", "companion_chip", "sensor_chip"];
    let titles = ["Main Chip OTA", "Companion Chip OTA", "Sensor Chip OTA"];

    for (i, (&chip_name, &title)) in chips.iter().zip(titles.iter()).enumerate() {
        draw_chip_section(chip_chunks[i], buf, title, chip_name, firmware)?;
    }

    Ok(())
}

fn draw_info_section(area: Rect, buf: &mut Buffer, firmware: &FirmwareProperty) -> Result<(), DMError> {
    let info_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

    // Draw req_info section
    let req_block = Block::default()
        .title(" Request Info ")
        .borders(Borders::ALL)
        .border_set(border::PLAIN);

    let req_inner = req_block.inner(info_chunks[0]);
    req_block.render(info_chunks[0], buf);

    let req_items = vec![
        format!("Req ID: {}", if firmware.req_info.req_id.is_empty() { "N/A" } else { &firmware.req_info.req_id }),
        format!("Version: {}", if firmware.version.is_empty() { "N/A" } else { &firmware.version }),
    ];

    let req_list_items: Vec<ListItem> = req_items
        .iter()
        .map(|item| ListItem::new(Line::from(Span::raw(item.as_str()))))
        .collect();

    List::new(req_list_items)
        .style(Style::default())
        .render(req_inner, buf);

    // Draw res_info section
    let res_block = Block::default()
        .title(" Response Info ")
        .borders(Borders::ALL)
        .border_set(border::PLAIN);

    let res_inner = res_block.inner(info_chunks[1]);
    res_block.render(info_chunks[1], buf);

    let res_items = vec![
        format!("Res ID: {}", if firmware.res_info.res_id.is_empty() { "N/A" } else { &firmware.res_info.res_id }),
        format!("Code: {:?}", firmware.res_info.code),
        format!("Detail: {}", if firmware.res_info.detail_msg.is_empty() { "N/A" } else { &firmware.res_info.detail_msg }),
    ];

    let res_list_items: Vec<ListItem> = res_items
        .iter()
        .map(|item| ListItem::new(Line::from(Span::raw(item.as_str()))))
        .collect();

    List::new(res_list_items)
        .style(Style::default())
        .render(res_inner, buf);

    Ok(())
}

fn draw_chip_section(
    area: Rect,
    buf: &mut Buffer,
    title: &str,
    chip_name: &str,
    firmware: &FirmwareProperty,
) -> Result<(), DMError> {
    let block = Block::default()
        .title(title)
        .borders(Borders::NONE)
        .title_alignment(Alignment::Right);

    let inner_area = block.inner(area);
    block.render(area, buf);

    // Split horizontally into loader and firmware subsections
    let subsections = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner_area);

    // Draw loader subsection
    if let Some(loader_target) = firmware.get_target(chip_name, Component::Loader) {
        draw_component_subsection(subsections[0], buf, " Loader ", loader_target)?;
    } else {
        draw_empty_component_subsection(subsections[0], buf, " Loader ")?;
    }

    // Draw firmware subsection
    if let Some(firmware_target) = firmware.get_target(chip_name, Component::Firmware) {
        draw_component_subsection(subsections[1], buf, " Firmware ", firmware_target)?;
    } else {
        draw_empty_component_subsection(subsections[1], buf, " Firmware ")?;
    }

    Ok(())
}

fn draw_component_subsection(
    area: Rect,
    buf: &mut Buffer,
    title: &str,
    target: &Target,
) -> Result<(), DMError> {
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_set(border::PLAIN);

    let inner_area = block.inner(area);
    block.render(area, buf);

    let items = vec![
        format!(
            "Version: {}",
            if target.version.is_empty() {
                "N/A"
            } else {
                &target.version
            }
        ),
        format!("Progress: {}%", target.progress),
        format!("State: {}", format_process_state(&target.process_state)),
        format!(
            "URL: {}",
            if target.package_url.is_empty() {
                "N/A"
            } else {
                &target.package_url
            }
        ),
        format!(
            "Hash: {}",
            if target.hash.is_empty() {
                "N/A"
            } else {
                &target.hash
            }
        ),
        format!("Size: {} bytes", target.size),
    ];

    let list_items: Vec<ListItem> = items
        .iter()
        .map(|item| ListItem::new(Line::from(Span::raw(item.as_str()))))
        .collect();

    let list = List::new(list_items).style(Style::default());

    list.render(inner_area, buf);

    Ok(())
}

fn draw_empty_component_subsection(
    area: Rect,
    buf: &mut Buffer,
    title: &str,
) -> Result<(), DMError> {
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_set(border::PLAIN);

    let inner_area = block.inner(area);
    block.render(area, buf);

    Paragraph::new("No data available")
        .style(Style::default().fg(Color::DarkGray))
        .render(inner_area, buf);

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
