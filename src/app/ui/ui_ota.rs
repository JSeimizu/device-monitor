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
    crate::{
        app::App,
        error::DMError,
        mqtt_ctrl::evp::ProcessState,
        mqtt_ctrl::with_mqtt_ctrl,
        ota::{ChipId, Component, FirmwareProperty, Target},
    },
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

pub fn draw(area: Rect, buf: &mut Buffer, _app: &App) -> Result<(), DMError> {
    with_mqtt_ctrl(|mqtt_ctrl| -> Result<(), DMError> {
        let firmware = mqtt_ctrl.firmware();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(6), // ReqInfo/ResInfo section
                Constraint::Min(0),    // Chip sections
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

        let chips = [ChipId::MainChip, ChipId::CompanionChip, ChipId::SensorChip];
        let titles = ["Main Chip OTA", "Companion Chip OTA", "Sensor Chip OTA"];

        for (i, (&chip_id, &title)) in chips.iter().zip(titles.iter()).enumerate() {
            draw_chip_section(chip_chunks[i], buf, title, chip_id, firmware)?;
        }

        Ok(())
    })
}

fn draw_info_section(
    area: Rect,
    buf: &mut Buffer,
    firmware: &FirmwareProperty,
) -> Result<(), DMError> {
    let info_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Draw req_info section
    let req_block = Block::default()
        .title(" Request Info ")
        .borders(Borders::ALL)
        .border_set(border::PLAIN);

    let req_inner = req_block.inner(info_chunks[0]);
    req_block.render(info_chunks[0], buf);

    let req_items = [
        format!(
            "Req ID: {}",
            if firmware.req_info.is_none() {
                "N/A"
            } else {
                firmware.req_info.as_ref().unwrap().req_id.as_str()
            }
        ),
        format!(
            "Version: {}",
            if firmware.version.is_none() {
                "N/A"
            } else {
                firmware.version.as_ref().unwrap()
            }
        ),
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

    let res_items = [
        format!(
            "Res ID: {}",
            if firmware.res_info.is_none() {
                "N/A"
            } else {
                firmware.res_info.as_ref().unwrap().res_id()
            }
        ),
        format!(
            "Code: {}",
            if firmware.res_info.is_none() {
                "N/A".to_owned()
            } else {
                firmware.res_info.as_ref().unwrap().code_str().to_string()
            }
        ),
        format!(
            "Detail: {}",
            if firmware.res_info.is_none() {
                "N/A"
            } else {
                firmware.res_info.as_ref().unwrap().detail_msg()
            }
        ),
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
    chip_id: ChipId,
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
    if let Some(loader_target) = firmware.get_target(chip_id, Component::Loader) {
        draw_component_subsection(subsections[0], buf, " Loader ", loader_target)?;
    } else {
        draw_empty_component_subsection(subsections[0], buf, " Loader ")?;
    }

    // Draw firmware subsection
    if let Some(firmware_target) = firmware.get_target(chip_id, Component::Firmware) {
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

    let items = [
        format!(
            "Chip: {}",
            if target.chip.is_empty() {
                "N/A"
            } else {
                &target.chip
            }
        ),
        format!(
            "Version: {}",
            if target.version.is_none() {
                "N/A"
            } else {
                target.version.as_ref().unwrap()
            }
        ),
        format!(
            "Progress: {}%",
            if target.progress.is_none() {
                "N/A".to_string()
            } else {
                target.progress.as_ref().unwrap().to_string()
            }
        ),
        format!("State: {}", {
            let state = target.process_state.as_ref().unwrap_or(&ProcessState::Idle);
            format_process_state(state)
        }),
        format!(
            "URL: {}",
            if target.package_url.is_none() {
                "N/A"
            } else {
                target.package_url.as_ref().unwrap()
            }
        ),
        format!(
            "Hash: {}",
            if target.hash.is_none() {
                "N/A"
            } else {
                target.hash.as_ref().unwrap()
            }
        ),
        format!("Size: {} bytes", {
            if target.size.is_none() {
                "N/A".to_string()
            } else {
                target.size.as_ref().unwrap().to_string()
            }
        }),
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
