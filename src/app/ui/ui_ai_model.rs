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
        ai_model::{AiModel, Target},
        app::App,
        error::DMError,
        mqtt_ctrl::{evp::ProcessState, with_mqtt_ctrl},
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
        let ai_model = mqtt_ctrl.ai_model();

        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(6), // Top row: ReqInfo/ResInfo
                Constraint::Min(0),    // Bottom: Target sections
            ])
            .split(area);

        // Draw req_info and res_info section
        draw_info_section(main_chunks[0], buf, ai_model)?;

        // Draw target sections in 2x2 grid
        draw_targets_section(main_chunks[1], buf, ai_model)?;

        Ok(())
    })
}

fn draw_info_section(area: Rect, buf: &mut Buffer, ai_model: &AiModel) -> Result<(), DMError> {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(0)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Draw req_info
    draw_req_info(chunks[0], buf, ai_model)?;

    // Draw res_info
    draw_res_info(chunks[1], buf, ai_model)?;

    Ok(())
}

fn draw_req_info(area: Rect, buf: &mut Buffer, ai_model: &AiModel) -> Result<(), DMError> {
    let block = Block::default()
        .title("Request Info")
        .borders(Borders::ALL)
        .border_set(border::THICK);

    let req_id = ai_model.req_info().map(|r| r.req_id.as_str()).unwrap_or("");

    let text = Text::from(vec![Line::from(vec![
        Span::styled("Req ID: ", Style::default().fg(Color::Cyan)),
        Span::raw(req_id),
    ])]);

    let paragraph = Paragraph::new(text).block(block).alignment(Alignment::Left);

    paragraph.render(area, buf);
    Ok(())
}

fn draw_res_info(area: Rect, buf: &mut Buffer, ai_model: &AiModel) -> Result<(), DMError> {
    let block = Block::default()
        .title("Response Info")
        .borders(Borders::ALL)
        .border_set(border::THICK);

    let (res_id, code, detail_msg) = if let Some(res_info) = ai_model.res_info() {
        (
            res_info.res_id(),
            res_info.code_str(),
            res_info.detail_msg(),
        )
    } else {
        ("", "", "")
    };

    let text = Text::from(vec![
        Line::from(vec![
            Span::styled("Res ID: ", Style::default().fg(Color::Cyan)),
            Span::raw(res_id),
        ]),
        Line::from(vec![
            Span::styled("Code: ", Style::default().fg(Color::Cyan)),
            Span::raw(code),
        ]),
        Line::from(vec![
            Span::styled("Detail: ", Style::default().fg(Color::Cyan)),
            Span::raw(detail_msg),
        ]),
    ]);

    let paragraph = Paragraph::new(text).block(block).alignment(Alignment::Left);

    paragraph.render(area, buf);
    Ok(())
}

fn draw_targets_section(area: Rect, buf: &mut Buffer, ai_model: &AiModel) -> Result<(), DMError> {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Top row: Target 1 | Target 2
    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(0)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[0]);

    // Bottom row: Target 3 | Target 4
    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(0)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    let targets = ai_model.targets();

    // Draw Target 1
    draw_target(top_chunks[0], buf, targets.first(), 1)?;

    // Draw Target 2
    draw_target(top_chunks[1], buf, targets.get(1), 2)?;

    // Draw Target 3
    draw_target(bottom_chunks[0], buf, targets.get(2), 3)?;

    // Draw Target 4
    draw_target(bottom_chunks[1], buf, targets.get(3), 4)?;

    Ok(())
}

fn draw_target(
    area: Rect,
    buf: &mut Buffer,
    target: Option<&Target>,
    index: usize,
) -> Result<(), DMError> {
    let block = Block::default()
        .title(format!("Target {}", index))
        .borders(Borders::ALL)
        .border_set(border::THICK);

    let text = if let Some(target) = target {
        let chip = target.chip.as_deref().unwrap_or("");
        let version = target.version.as_deref().unwrap_or("");
        let progress = target.progress.unwrap_or(0);
        let process_state = format_process_state(target.process_state.as_ref());
        let package_url = target.package_url.as_deref().unwrap_or("");
        let hash = target.hash.as_deref().unwrap_or("");
        let size = target.size.unwrap_or(0);

        Text::from(vec![
            Line::from(vec![
                Span::styled("Chip: ", Style::default().fg(Color::Cyan)),
                Span::raw(chip),
            ]),
            Line::from(vec![
                Span::styled("Version: ", Style::default().fg(Color::Cyan)),
                Span::raw(version),
            ]),
            Line::from(vec![
                Span::styled("Progress: ", Style::default().fg(Color::Cyan)),
                Span::raw(format!("{}%", progress)),
            ]),
            Line::from(vec![
                Span::styled("State: ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    process_state,
                    get_state_color(target.process_state.as_ref()),
                ),
            ]),
            Line::from(vec![
                Span::styled("URL: ", Style::default().fg(Color::Cyan)),
                Span::raw(if package_url.len() > 30 {
                    format!("{}...", &package_url[..27])
                } else {
                    package_url.to_string()
                }),
            ]),
            Line::from(vec![
                Span::styled("Hash: ", Style::default().fg(Color::Cyan)),
                Span::raw(if hash.len() > 20 {
                    format!("{}...", &hash[..17])
                } else {
                    hash.to_string()
                }),
            ]),
            Line::from(vec![
                Span::styled("Size: ", Style::default().fg(Color::Cyan)),
                Span::raw(format!("{} bytes", size)),
            ]),
        ])
    } else {
        Text::from("No target data")
    };

    let paragraph = Paragraph::new(text).block(block).alignment(Alignment::Left);

    paragraph.render(area, buf);
    Ok(())
}

fn format_process_state(state: Option<&ProcessState>) -> String {
    match state {
        Some(ProcessState::Idle) => "idle".to_string(),
        Some(ProcessState::RequestReceived) => "request_received".to_string(),
        Some(ProcessState::Downloading) => "downloading".to_string(),
        Some(ProcessState::Installing) => "installing".to_string(),
        Some(ProcessState::Done) => "done".to_string(),
        Some(ProcessState::Failed) => "failed".to_string(),
        Some(ProcessState::FailedInvalidArgument) => "failed_invalid_argument".to_string(),
        Some(ProcessState::FailedTokenExpired) => "failed_token_expired".to_string(),
        Some(ProcessState::FailedDownloadRetryExceeded) => {
            "failed_download_retry_exceeded".to_string()
        }
        None => "unknown".to_string(),
    }
}

fn get_state_color(state: Option<&ProcessState>) -> Style {
    match state {
        Some(ProcessState::Idle) => Style::default().fg(Color::Gray),
        Some(ProcessState::RequestReceived) => Style::default().fg(Color::Yellow),
        Some(ProcessState::Downloading) => Style::default().fg(Color::Blue),
        Some(ProcessState::Installing) => Style::default().fg(Color::Magenta),
        Some(ProcessState::Done) => Style::default().fg(Color::Green),
        Some(ProcessState::Failed)
        | Some(ProcessState::FailedInvalidArgument)
        | Some(ProcessState::FailedTokenExpired)
        | Some(ProcessState::FailedDownloadRetryExceeded) => Style::default().fg(Color::Red),
        None => Style::default().fg(Color::Gray),
    }
}
