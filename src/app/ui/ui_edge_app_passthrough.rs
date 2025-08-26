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
    app::{
        App, DMScreen, DMScreenState, EdgeAppConfigBlock, EdgeAppNavigationMode, ui::normal_block,
    },
    error::DMError,
    mqtt_ctrl::{evp::edge_app_passthrough::EdgeAppPassthrough, with_mqtt_ctrl},
};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::{Color, Style},
    style::Stylize,
    text::{Line, Span},
    widgets::{Borders, List, ListItem, Widget},
};

fn list_items_push_str(list_items: &mut Vec<ListItem>, key: &str, value: &str) {
    list_items.push(ListItem::new(Line::from(vec![
        Span::styled(key.to_string(), Style::default().fg(Color::Blue)),
        Span::raw(": "),
        Span::styled(value.to_string(), Style::default().fg(Color::White)),
    ])));
}

fn list_items_push_number(list_items: &mut Vec<ListItem>, key: &str, value: Option<i32>) {
    let key_owned = key.to_string();
    let value_str = match value {
        Some(v) => v.to_string(),
        None => "none".to_string(),
    };
    list_items.push(ListItem::new(Line::from(vec![
        Span::styled(key_owned, Style::default().fg(Color::Blue)),
        Span::raw(": "),
        Span::styled(value_str, Style::default().fg(Color::White)),
    ])));
}

fn list_items_push_float(list_items: &mut Vec<ListItem>, key: &str, value: Option<f64>) {
    let key_owned = key.to_string();
    let value_str = match value {
        Some(v) => v.to_string(),
        None => "none".to_string(),
    };
    list_items.push(ListItem::new(Line::from(vec![
        Span::styled(key_owned, Style::default().fg(Color::Blue)),
        Span::raw(": "),
        Span::styled(value_str, Style::default().fg(Color::White)),
    ])));
}

fn list_items_push_bool(list_items: &mut Vec<ListItem>, key: &str, value: Option<bool>) {
    let key_owned = key.to_string();
    let value_str = match value {
        Some(v) => v.to_string(),
        None => "none".to_string(),
    };
    list_items.push(ListItem::new(Line::from(vec![
        Span::styled(key_owned, Style::default().fg(Color::Blue)),
        Span::raw(": "),
        Span::styled(value_str, Style::default().fg(Color::White)),
    ])));
}

fn list_items_push_string(list_items: &mut Vec<ListItem>, key: &str, value: &Option<String>) {
    let key_owned = key.to_string();
    let value_str = match value {
        Some(v) => v.clone(),
        None => "none".to_string(),
    };
    list_items.push(ListItem::new(Line::from(vec![
        Span::styled(key_owned, Style::default().fg(Color::Blue)),
        Span::raw(": "),
        Span::styled(value_str, Style::default().fg(Color::White)),
    ])));
}

fn list_items_push_section_header(list_items: &mut Vec<ListItem>, title: &str) {
    list_items.push(ListItem::new(Line::from(vec![Span::styled(
        title.to_string(),
        Style::default().fg(Color::Yellow).bold(),
    )])));
}

pub fn draw(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    // Check current state
    if let DMScreen::EdgeAppPassthrough(state) = app.current_screen() {
        match state {
            DMScreenState::Initial => draw_initial_state(area, buf, app),
            DMScreenState::Configuring => draw_configuring_state(area, buf, app),
            DMScreenState::Completed => draw_completed_state(area, buf, app),
        }
    } else {
        // Fallback to initial state
        draw_initial_state(area, buf, app)
    }
}

fn draw_initial_state(area: Rect, buf: &mut Buffer, _app: &App) -> Result<(), DMError> {
    with_mqtt_ctrl(|mqtt_ctrl| -> Result<(), DMError> {
        let edge_app = mqtt_ctrl.edge_app_passthrough();

        let outer_block = normal_block("EdgeApp Passthrough").borders(Borders::NONE);
        let inner_area = outer_block.inner(area);
        outer_block.render(area, buf);

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(0)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(inner_area);

        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
            .split(chunks[0]);

        // Left top: Request & Response Info
        {
            let mut list_items = Vec::<ListItem>::new();

            // Request Info
            list_items_push_section_header(&mut list_items, "Request Info");
            if let Some(req_info) = &edge_app.req_info {
                list_items_push_string(&mut list_items, "  req_id", &req_info.req_id);
            } else {
                list_items_push_str(&mut list_items, "  req_id", "none");
            }

            list_items.push(ListItem::new(Line::from("")));

            // Response Info
            list_items_push_section_header(&mut list_items, "Response Info");
            if let Some(res_info) = &edge_app.res_info {
                list_items_push_string(&mut list_items, "  res_id", &res_info.res_id);
                list_items_push_str(&mut list_items, "  code", edge_app.get_response_code_str());
                list_items_push_string(&mut list_items, "  detail_msg", &res_info.detail_msg);
            } else {
                list_items_push_str(&mut list_items, "  res_id", "none");
                list_items_push_str(&mut list_items, "  code", "none");
                list_items_push_str(&mut list_items, "  detail_msg", "none");
            }

            let req_res_block = normal_block("Request & Response");
            List::new(list_items)
                .block(req_res_block)
                .render(left_chunks[0], buf);
        }

        // Left bottom: Common Settings Overview
        {
            let mut list_items = Vec::<ListItem>::new();

            if let Some(common_settings) = &edge_app.common_settings {
                list_items_push_section_header(&mut list_items, "Basic Settings");
                list_items.push(ListItem::new(Line::from(vec![
                    Span::styled("  process_state", Style::default().fg(Color::Blue)),
                    Span::raw(": "),
                    Span::styled(
                        edge_app.get_process_state_str(),
                        Style::default().fg(Color::White),
                    ),
                ])));
                list_items.push(ListItem::new(Line::from(vec![
                    Span::styled("  log_level", Style::default().fg(Color::Blue)),
                    Span::raw(": "),
                    Span::styled(
                        edge_app.get_log_level_str(),
                        Style::default().fg(Color::White),
                    ),
                ])));

                if let Some(inference_settings) = &common_settings.inference_settings {
                    list_items_push_number(
                        &mut list_items,
                        "  number_of_iterations",
                        inference_settings.number_of_iterations,
                    );
                }

                list_items_push_number(
                    &mut list_items,
                    "  inference_per_message",
                    common_settings.number_of_inference_per_message,
                );

                list_items.push(ListItem::new(Line::from("")));

                // PQ Settings Overview
                if let Some(pq_settings) = &common_settings.pq_settings {
                    list_items_push_section_header(&mut list_items, "PQ Settings");

                    if let Some(camera_image_size) = &pq_settings.camera_image_size {
                        list_items_push_number(&mut list_items, "  width", camera_image_size.width);
                        list_items_push_number(
                            &mut list_items,
                            "  height",
                            camera_image_size.height,
                        );
                        list_items.push(ListItem::new(Line::from(vec![
                            Span::styled("  scaling_policy", Style::default().fg(Color::Blue)),
                            Span::raw(": "),
                            Span::styled(
                                edge_app.get_scaling_policy_str(),
                                Style::default().fg(Color::White),
                            ),
                        ])));
                    }

                    if let Some(frame_rate) = &pq_settings.frame_rate {
                        list_items_push_number(&mut list_items, "  frame_rate_num", frame_rate.num);
                        list_items_push_number(
                            &mut list_items,
                            "  frame_rate_denom",
                            frame_rate.denom,
                        );
                    }

                    list_items_push_float(
                        &mut list_items,
                        "  digital_zoom",
                        pq_settings.digital_zoom,
                    );
                    list_items.push(ListItem::new(Line::from(vec![
                        Span::styled("  exposure_mode", Style::default().fg(Color::Blue)),
                        Span::raw(": "),
                        Span::styled(
                            edge_app.get_exposure_mode_str(),
                            Style::default().fg(Color::White),
                        ),
                    ])));
                    list_items.push(ListItem::new(Line::from(vec![
                        Span::styled("  white_balance_mode", Style::default().fg(Color::Blue)),
                        Span::raw(": "),
                        Span::styled(
                            edge_app.get_white_balance_mode_str(),
                            Style::default().fg(Color::White),
                        ),
                    ])));
                }

                // Codec Settings
                if let Some(_codec_settings) = &common_settings.codec_settings {
                    list_items.push(ListItem::new(Line::from("")));
                    list_items_push_section_header(&mut list_items, "Codec Settings");
                    list_items.push(ListItem::new(Line::from(vec![
                        Span::styled("  format", Style::default().fg(Color::Blue)),
                        Span::raw(": "),
                        Span::styled(
                            edge_app.get_codec_format_str(),
                            Style::default().fg(Color::White),
                        ),
                    ])));
                }
            } else {
                list_items_push_str(&mut list_items, "No common settings", "");
            }

            let common_settings_block = normal_block("Common Settings");
            List::new(list_items)
                .block(common_settings_block)
                .render(left_chunks[1], buf);
        }

        // Right side: Detailed Settings
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(chunks[1]);

        // Right top: Port Settings
        {
            let mut list_items = Vec::<ListItem>::new();

            if let Some(common_settings) = &edge_app.common_settings {
                if let Some(port_settings) = &common_settings.port_settings {
                    list_items_push_section_header(&mut list_items, "Metadata Port");
                    if let Some(metadata) = &port_settings.metadata {
                        list_items_push_str(
                            &mut list_items,
                            "  method",
                            EdgeAppPassthrough::get_port_method_str(metadata.method),
                        );
                        list_items_push_string(
                            &mut list_items,
                            "  storage_name",
                            &metadata.storage_name,
                        );
                        list_items_push_string(&mut list_items, "  endpoint", &metadata.endpoint);
                        list_items_push_string(&mut list_items, "  path", &metadata.path);
                        list_items_push_bool(&mut list_items, "  enabled", metadata.enabled);
                    } else {
                        list_items_push_str(&mut list_items, "  method", "none");
                        list_items_push_str(&mut list_items, "  enabled", "none");
                    }

                    list_items.push(ListItem::new(Line::from("")));

                    list_items_push_section_header(&mut list_items, "Input Tensor Port");
                    if let Some(input_tensor) = &port_settings.input_tensor {
                        list_items_push_str(
                            &mut list_items,
                            "  method",
                            EdgeAppPassthrough::get_port_method_str(input_tensor.method),
                        );
                        list_items_push_string(
                            &mut list_items,
                            "  storage_name",
                            &input_tensor.storage_name,
                        );
                        list_items_push_string(
                            &mut list_items,
                            "  endpoint",
                            &input_tensor.endpoint,
                        );
                        list_items_push_string(&mut list_items, "  path", &input_tensor.path);
                        list_items_push_bool(&mut list_items, "  enabled", input_tensor.enabled);
                    } else {
                        list_items_push_str(&mut list_items, "  method", "none");
                        list_items_push_str(&mut list_items, "  enabled", "none");
                    }
                } else {
                    list_items_push_str(&mut list_items, "No port settings", "");
                }
            } else {
                list_items_push_str(&mut list_items, "No common settings", "");
            }

            let port_settings_block = normal_block("Port Settings");
            List::new(list_items)
                .block(port_settings_block)
                .render(right_chunks[0], buf);
        }

        // Right bottom: Custom Settings
        {
            let mut list_items = Vec::<ListItem>::new();

            if let Some(custom_settings) = &edge_app.custom_settings {
                list_items_push_section_header(&mut list_items, "Custom Response Info");
                if let Some(res_info) = &custom_settings.res_info {
                    list_items_push_string(&mut list_items, "  res_id", &res_info.res_id);
                    let code_str = match res_info.code {
                        Some(0) => "ok",
                        Some(1) => "cancelled",
                        Some(2) => "unknown",
                        Some(3) => "invalid_argument",
                        Some(4) => "deadline_exceeded",
                        Some(5) => "not_found",
                        Some(6) => "already_exists",
                        Some(7) => "permission_denied",
                        Some(8) => "resource_exhausted",
                        Some(9) => "failed_precondition",
                        Some(10) => "aborted",
                        Some(11) => "out_of_range",
                        Some(12) => "unimplemented",
                        Some(13) => "internal",
                        Some(14) => "unavailable",
                        Some(15) => "data_loss",
                        Some(16) => "unauthenticated",
                        Some(code) => &format!("unknown({})", code),
                        None => "none",
                    };
                    list_items_push_str(&mut list_items, "  code", code_str);
                    list_items_push_string(&mut list_items, "  detail_msg", &res_info.detail_msg);
                } else {
                    list_items_push_str(&mut list_items, "  res_id", "none");
                    list_items_push_str(&mut list_items, "  code", "none");
                    list_items_push_str(&mut list_items, "  detail_msg", "none");
                }

                list_items.push(ListItem::new(Line::from("")));

                list_items_push_section_header(&mut list_items, "AI Models");
                if let Some(ai_models) = &custom_settings.ai_models {
                    if ai_models.is_empty() {
                        list_items_push_str(&mut list_items, "  (empty)", "");
                    } else {
                        for (name, bundle) in ai_models {
                            let bundle_id = match &bundle.ai_model_bundle_id {
                                Some(id) => id.as_str(),
                                None => "none",
                            };
                            list_items_push_str(&mut list_items, &format!("  {}", name), bundle_id);
                        }
                    }
                } else {
                    list_items_push_str(&mut list_items, "  (none)", "");
                }
            } else {
                list_items_push_str(&mut list_items, "No custom settings", "");
            }

            let custom_settings_block = normal_block("Custom Settings");
            List::new(list_items)
                .block(custom_settings_block)
                .render(right_chunks[1], buf);
        }

        Ok(())
    })
}

fn draw_configuring_state(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    let outer_block = normal_block("EdgeApp Passthrough - Configuration").borders(Borders::NONE);
    let inner_area = outer_block.inner(area);
    outer_block.render(area, buf);

    // Create layout for the blocks (2x3 grid, but CustomSettings is empty so effectively 2x2 + 1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(inner_area);

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(0)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(chunks[0]);

    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(0)
        .constraints(
            [
                Constraint::Percentage(33),
                Constraint::Percentage(33),
                Constraint::Percentage(34),
            ]
            .as_ref(),
        )
        .split(chunks[1]);

    // Block areas mapping
    let block_areas = [
        top_chunks[0],    // CommonSettings
        top_chunks[1],    // PQSettings
        bottom_chunks[0], // PortSettings
        bottom_chunks[1], // CodecSettings
        bottom_chunks[2], // CustomSettings
    ];

    let blocks = EdgeAppConfigBlock::all();

    for (block_index, block) in blocks.iter().enumerate() {
        let area = block_areas[block_index];
        let is_focused_block = app.edge_app_block_focus == block_index;
        let is_in_field_mode =
            app.edge_app_navigation_mode == EdgeAppNavigationMode::Field && is_focused_block;

        // Block highlighting
        let block_style = if is_focused_block {
            if is_in_field_mode {
                normal_block(block.title()).style(Style::default().fg(Color::Green))
            } else {
                normal_block(block.title()).style(Style::default().fg(Color::Yellow))
            }
        } else {
            normal_block(block.title())
        };

        let block_inner = block_style.inner(area);
        block_style.render(area, buf);

        // Draw block content
        draw_block_content(block_inner, buf, app, *block, block_index, is_in_field_mode)?;
    }

    Ok(())
}

fn draw_block_content(
    area: Rect,
    buf: &mut Buffer,
    app: &App,
    block: EdgeAppConfigBlock,
    block_index: usize,
    is_in_field_mode: bool,
) -> Result<(), DMError> {
    let config_keys = block.get_config_keys();
    if config_keys.is_empty() {
        // Empty block (like CustomSettings)
        let empty_text = ListItem::new(Line::from(vec![Span::styled(
            "No configuration fields",
            Style::default().fg(Color::Gray),
        )]));
        List::new(vec![empty_text]).render(area, buf);
        return Ok(());
    }

    let mut list_items = Vec::<ListItem>::new();
    let current_field_focus = app
        .edge_app_field_focus
        .get(block_index)
        .copied()
        .unwrap_or(0);
    let scroll_offset = app
        .edge_app_field_scroll
        .get(block_index)
        .copied()
        .unwrap_or(0);

    // Calculate visible window for scrolling
    let visible_height = area.height.saturating_sub(2) as usize; // Account for borders
    let total_fields = config_keys.len();

    // Determine which fields are visible based on scroll offset
    let visible_start = scroll_offset.min(total_fields.saturating_sub(1));
    let visible_end = (visible_start + visible_height).min(total_fields);

    // Only create list items for visible fields
    for field_index in visible_start..visible_end {
        if let Some(&config_key) = config_keys.get(field_index) {
            let is_focused_field = is_in_field_mode && field_index == current_field_focus;
            let config_key_index = usize::from(config_key);
            let is_editable = app.config_key_editable && is_focused_field;

            let default_string = String::new();
            let value = app
                .config_keys
                .get(config_key_index)
                .unwrap_or(&default_string);
            let field_name = format!("{}", config_key);

            let field_style = if is_focused_field {
                if is_editable {
                    Style::default().fg(Color::Green).bold()
                } else {
                    Style::default().fg(Color::Yellow).bold()
                }
            } else {
                Style::default().fg(Color::Blue)
            };

            let value_style = if is_editable {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::White)
            };

            list_items.push(ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(field_name, field_style),
                Span::raw(": "),
                Span::styled(value.clone(), value_style),
                if is_editable {
                    Span::raw(" ◄")
                } else {
                    Span::raw("")
                },
            ])));
        }
    }

    // Add scroll indicators if needed
    if total_fields > visible_height {
        if scroll_offset > 0 {
            list_items.insert(
                0,
                ListItem::new(Line::from(vec![Span::styled(
                    "▲ More fields above",
                    Style::default().fg(Color::Gray),
                )])),
            );
        }
        if visible_end < total_fields {
            list_items.push(ListItem::new(Line::from(vec![Span::styled(
                "▼ More fields below",
                Style::default().fg(Color::Gray),
            )])));
        }
    }

    List::new(list_items).render(area, buf);

    Ok(())
}

fn draw_completed_state(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    let outer_block =
        normal_block("EdgeApp Passthrough - Configuration Result").borders(Borders::NONE);
    let inner_area = outer_block.inner(area);
    outer_block.render(area, buf);

    let mut list_items = Vec::<ListItem>::new();

    if let Some(config_result) = &app.config_result {
        match config_result {
            Ok(json_string) => {
                //list_items_push_section_header(&mut list_items, "Configuration JSON");
                // Split the JSON string into lines for better display
                for line in json_string.lines() {
                    list_items.push(ListItem::new(Line::from(vec![Span::styled(
                        line.to_string(),
                        Style::default().fg(Color::White),
                    )])));
                }
            }
            Err(error) => {
                list_items_push_section_header(&mut list_items, "Error");
                list_items.push(ListItem::new(Line::from(vec![Span::styled(
                    format!("{:?}", error),
                    Style::default().fg(Color::Red),
                )])));
            }
        }
    } else {
        list_items.push(ListItem::new(Line::from(vec![Span::styled(
            "No configuration result",
            Style::default().fg(Color::Yellow),
        )])));
    }

    let result_block = normal_block("");
    List::new(list_items)
        .block(result_block)
        .render(inner_area, buf);

    Ok(())
}
