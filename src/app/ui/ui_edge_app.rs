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

use crate::mqtt_ctrl::with_mqtt_ctrl;
#[allow(unused)]
use {
    super::{
        list_items_push, list_items_push_blank, list_items_push_focus, list_items_push_text_focus,
    },
    crate::{
        app::{App, ConfigKey, DMScreen, DMScreenState, ui::focus_block, ui::normal_block},
        error::DMError,
    },
    json::{JsonValue, object::Object},
    ratatui::{
        buffer::Buffer,
        layout::{Alignment, Rect},
        layout::{Constraint, Layout},
        prelude::{Backend, CrosstermBackend},
        prelude::{Color, Direction, Style},
        style::Stylize,
        symbols::border,
        text::{Line, Span, Text},
        widgets::{Block, Borders, List, ListItem, Paragraph, Widget},
    },
};

pub fn draw_default_state(area: Rect, buf: &mut Buffer, _app: &App) -> Result<(), DMError> {
    with_mqtt_ctrl(|mqtt_ctrl| -> Result<(), DMError> {
        if let Some(edge_app) = mqtt_ctrl.edge_app() {
            // Edge App should be included in the deployment status
            if let Some(deployment_status) = mqtt_ctrl.deployment_status() {
                if !deployment_status
                    .instances()
                    .iter()
                    .any(|(id, _)| id.uuid() == edge_app.id())
                {
                    return Ok(());
                }
            }

            let title = format!("Edge App: {}", edge_app.id());
            let outer_block = normal_block(&title).borders(Borders::NONE);

            let inner_area = outer_block.inner(area);
            outer_block.render(area, buf);

            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .margin(0)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
                .split(inner_area);

            // Common Settings
            {
                let common_settings = edge_app.module().common_settings();
                let mut list_items = Vec::<ListItem>::new();

                // Process state
                if let Some(process_state) = common_settings.process_state() {
                    list_items_push(&mut list_items, "process_state", &process_state.to_string());
                }

                // Log Level
                if let Some(log_level) = common_settings.log_level() {
                    list_items_push(&mut list_items, "log_level", &log_level.to_string());
                }

                // Inference Settings
                if let Some(inference_settings) = common_settings.inference_settings() {
                    list_items_push_text_focus(&mut list_items, "inference_settings", false);
                    if let Some(number_of_iterations) = inference_settings.number_of_iterations() {
                        list_items_push(
                            &mut list_items,
                            "  number_of_iterations",
                            number_of_iterations.to_string().as_str(),
                        );
                    }
                }

                // PQ Settings
                if let Some(pq_settings) = common_settings.pq_settings() {
                    list_items_push_text_focus(&mut list_items, "pq_settings", false);
                    if let Some(camera_image_size) = pq_settings.camera_image_size() {
                        list_items_push(
                            &mut list_items,
                            "  camera_image_size",
                            camera_image_size.to_string().as_str(),
                        );
                    }

                    if let Some(frame_rate) = pq_settings.frame_rate() {
                        list_items_push(
                            &mut list_items,
                            "  frame_rate",
                            frame_rate.to_string().as_str(),
                        );
                    }

                    if let Some(digital_zoom) = pq_settings.digital_zoom() {
                        list_items_push(
                            &mut list_items,
                            "  digital_zoom",
                            digital_zoom.to_string().as_str(),
                        );
                    }

                    if let Some(camera_image_flip) = pq_settings.camera_image_flip() {
                        list_items_push(
                            &mut list_items,
                            "  camera_image_flip",
                            camera_image_flip.to_string().as_str(),
                        );
                    }

                    // Exposure Mode
                    if let Some(exposure_mode) = pq_settings.exposure_mode() {
                        let mode = match exposure_mode {
                            0 => "auto",
                            1 => "manual",
                            _ => "invalid",
                        };

                        list_items_push(
                            &mut list_items,
                            "  exposure_mode",
                            format!("{mode} ({exposure_mode})").as_str(),
                        );

                        // If exposure mode is auto, show auto exposure settings
                        if exposure_mode == 0 {
                            // Auto Exposure Settings
                            if let Some(auto_exposure) = pq_settings.auto_exposure() {
                                list_items_push_text_focus(
                                    &mut list_items,
                                    "  auto_exposure",
                                    false,
                                );

                                if let Some(max_exposure_time) = auto_exposure.max_exposure_time() {
                                    list_items_push(
                                        &mut list_items,
                                        "    max_exposure_time",
                                        max_exposure_time.to_string().as_str(),
                                    );
                                }

                                if let Some(min_exposure_time) = auto_exposure.min_exposure_time() {
                                    list_items_push(
                                        &mut list_items,
                                        "    min_exposure_time",
                                        min_exposure_time.to_string().as_str(),
                                    );
                                }

                                if let Some(max_gain) = auto_exposure.max_gain() {
                                    list_items_push(
                                        &mut list_items,
                                        "    max_gain",
                                        max_gain.to_string().as_str(),
                                    );
                                }

                                if let Some(convergence_speed) = auto_exposure.convergence_speed() {
                                    list_items_push(
                                        &mut list_items,
                                        "    convergence_speed",
                                        convergence_speed.to_string().as_str(),
                                    );
                                }
                            }

                            // EV compensation
                            if let Some(ev_compensation) = pq_settings.ev_compensation() {
                                list_items_push(
                                    &mut list_items,
                                    "  ev_compensation: {}",
                                    ev_compensation.to_string().as_str(),
                                );
                            }

                            // AE Anti Flicker Mode
                            if let Some(ae_anti_flicker_mode) = pq_settings.ae_anti_flicker_mode() {
                                let mode = match ae_anti_flicker_mode {
                                    0 => "auto",
                                    1 => "50Hz",
                                    2 => "60Hz",
                                    _ => "invalid",
                                };
                                list_items_push(
                                    &mut list_items,
                                    "  ae_anti_flicker_mode",
                                    format!("{mode} ({ae_anti_flicker_mode})").as_str(),
                                );
                            }
                        }

                        if exposure_mode == 1 {
                            if let Some(manual_exposure) = pq_settings.manual_exposure() {
                                list_items_push_text_focus(
                                    &mut list_items,
                                    "  manual_exposure",
                                    false,
                                );

                                if let Some(exposure_time) = manual_exposure.exposure_time() {
                                    list_items_push(
                                        &mut list_items,
                                        "    exposure_time",
                                        exposure_time.to_string().as_str(),
                                    );
                                }

                                if let Some(gain) = manual_exposure.gain() {
                                    list_items_push(
                                        &mut list_items,
                                        "    gain",
                                        gain.to_string().as_str(),
                                    );
                                }
                            }
                        }
                    }

                    if let Some(white_balance_mode) = pq_settings.white_balance_mode() {
                        let mode = match white_balance_mode {
                            0 => "auto",
                            1 => "manual",
                            _ => "invalid",
                        };
                        list_items_push(
                            &mut list_items,
                            "  white_balance_mode",
                            format!("{mode} (white_balance_mode)").as_str(),
                        );

                        // If white balance mode is auto, show auto white balance settings
                        if white_balance_mode == 0 {
                            if let Some(auto_white_balance) = pq_settings.auto_white_balance() {
                                list_items_push_text_focus(
                                    &mut list_items,
                                    "  auto_white_balance",
                                    false,
                                );

                                if let Some(convergence_speed) =
                                    auto_white_balance.convergence_speed()
                                {
                                    list_items_push(
                                        &mut list_items,
                                        "    convergence_speed",
                                        convergence_speed.to_string().as_str(),
                                    );
                                }
                            }
                        }

                        // if white balance mode is preset
                        if white_balance_mode == 1 {
                            if let Some(manual_white_balance_preset) =
                                pq_settings.manual_white_balance_preset()
                            {
                                list_items_push_text_focus(
                                    &mut list_items,
                                    "  manual_white_balance",
                                    false,
                                );

                                if let Some(color_temperature) =
                                    manual_white_balance_preset.color_temperature()
                                {
                                    let color_temp = match color_temperature {
                                        0 => "3200K",
                                        1 => "4300K",
                                        2 => "5600K",
                                        3 => "6500K",
                                        _ => "invalid",
                                    };

                                    list_items_push(
                                        &mut list_items,
                                        "    color_temperature",
                                        format!("{} ({})", color_temp, color_temperature).as_str(),
                                    );
                                }
                            }
                        }
                    }

                    if let Some(image_cropping) = pq_settings.image_cropping() {
                        list_items_push(
                            &mut list_items,
                            "  image_cropping",
                            image_cropping.to_string().as_str(),
                        );
                    }

                    if let Some(image_rotation) = pq_settings.image_rotation() {
                        let image_rotation_str = match image_rotation {
                            0 => "0 degrees",
                            1 => "clockwise 90 degrees",
                            2 => "clockwise 180 degrees",
                            3 => "clockwise 270 degrees",
                            _ => "invalid",
                        };
                        list_items_push(
                            &mut list_items,
                            "  image_rotation",
                            &format!("{} ({})", image_rotation_str, image_rotation),
                        );
                    }

                    // Don's display registered settings
                }

                // Port Settings
                if let Some(port_settings) = common_settings.port_settings() {
                    list_items_push_text_focus(&mut list_items, "port_settings", false);
                    if let Some(metadata) = port_settings.metadata() {
                        list_items_push_text_focus(&mut list_items, "  metadata", false);
                        if let Some(method) = metadata.method() {
                            let m = match method {
                                0 => "EVP telemetry",
                                1 => "Blob storage",
                                2 => "Http storage",
                                _ => "Invalid",
                            };

                            list_items_push(
                                &mut list_items,
                                "    method",
                                format!("{} ({})", m, method).as_str(),
                            );
                        }

                        if let Some(storage_name) = metadata.storage_name() {
                            list_items_push(&mut list_items, "    storage_name", storage_name);
                        }

                        if let Some(endpoint) = metadata.endpoint() {
                            list_items_push(&mut list_items, "    endpoint", endpoint);
                        }

                        if let Some(path) = metadata.path() {
                            list_items_push(&mut list_items, "    path", path);
                        }

                        if let Some(enabled) = metadata.enabled() {
                            list_items_push(
                                &mut list_items,
                                "    enabled",
                                enabled.to_string().as_str(),
                            );
                        }
                    }

                    if let Some(input_tensor) = port_settings.input_tensor() {
                        list_items_push_text_focus(&mut list_items, "  input_tensor", false);
                        if let Some(method) = input_tensor.method() {
                            let m = match method {
                                0 => "EVP telemetry",
                                1 => "Blob storage",
                                2 => "Http storage",
                                _ => "Invalid",
                            };

                            list_items_push(
                                &mut list_items,
                                "    method",
                                format!("{} ({})", m, method).as_str(),
                            );
                        }

                        if let Some(storage_name) = input_tensor.storage_name() {
                            list_items_push(&mut list_items, "    storage_name", storage_name);
                        }

                        if let Some(endpoint) = input_tensor.endpoint() {
                            list_items_push(&mut list_items, "    endpoint", endpoint);
                        }

                        if let Some(path) = input_tensor.path() {
                            list_items_push(&mut list_items, "    path", path);
                        }

                        if let Some(enabled) = input_tensor.enabled() {
                            list_items_push(
                                &mut list_items,
                                "    enabled",
                                enabled.to_string().as_str(),
                            );
                        }
                    }
                }

                // Codec settings
                if let Some(codec_settings) = common_settings.codec_settings() {
                    list_items_push_text_focus(&mut list_items, "codec_settings", false);
                    if let Some(format) = codec_settings.format() {
                        let format_str = match format {
                            1 => "JPEG",
                            _ => "Invalid",
                        };
                        list_items_push(
                            &mut list_items,
                            "  codec",
                            &format!("{} ({})", format_str, format),
                        );
                    }
                }

                // Number of inference per message
                if let Some(number_of_inference_per_message) =
                    common_settings.number_of_inference_per_message()
                {
                    list_items_push(
                        &mut list_items,
                        "number_of_inference_per_message",
                        number_of_inference_per_message.to_string().as_str(),
                    );
                }

                // upload_interval
                if let Some(upload_interval) = common_settings.upload_interval() {
                    list_items_push(
                        &mut list_items,
                        "upload_interval",
                        upload_interval.to_string().as_str(),
                    );
                }

                let common_settings_block = normal_block("Common Settings");
                List::new(list_items)
                    .block(common_settings_block)
                    .render(chunks[0], buf);
            }

            let right_chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints([Constraint::Percentage(25), Constraint::Percentage(75)].as_ref())
                .split(chunks[1]);

            // Req/Res Info
            {
                let mut list_items = Vec::<ListItem>::new();

                if let Some(req_info) = edge_app.module().req_info() {
                    list_items_push_text_focus(&mut list_items, "req_info", false);
                    list_items_push(&mut list_items, "  req_id", req_info.req_id());
                }

                if let Some(res_info) = edge_app.module().res_info() {
                    list_items_push_text_focus(&mut list_items, "res_info", false);
                    list_items_push(&mut list_items, "  res_id", res_info.res_id());

                    list_items_push(&mut list_items, "  code", res_info.code_str());
                    list_items_push(&mut list_items, "  detail_msg", res_info.detail_msg());
                }

                let req_res_block = normal_block("Req/Res Info");
                List::new(list_items)
                    .block(req_res_block)
                    .render(right_chunks[0], buf);
            }

            // Custom Settings
            {
                let custom_settings_block = normal_block("Custom Settings");
                if let Some(custom_settings) = edge_app.module().custom_settings() {
                    if let Some(custom) = custom_settings.custom() {
                        Paragraph::new(custom.to_owned())
                            .block(custom_settings_block.clone())
                            .alignment(Alignment::Left)
                            .render(right_chunks[1], buf);
                    }
                }
            }
        }
        Ok(())
    })
}

pub fn draw_configure_state(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    let focus = |config_key| ConfigKey::from(app.config_key_focus) == config_key;

    let value = |config_key| {
        let value = app
            .config_keys
            .get(usize::from(config_key))
            .map(|s| s.as_str())
            .unwrap_or_default();

        if app.config_key_editable && focus(config_key) {
            format!("{}|", value)
        } else {
            value.to_string()
        }
    };

    let mut list_items = Vec::<ListItem>::new();

    for key in app.config_key_focus_start..app.config_key_focus_end {
        let config_key = ConfigKey::from(key);

        list_items_push_focus(
            &mut list_items,
            config_key.to_string().as_str(),
            &value(config_key),
            focus(config_key),
        );
    }

    list_items_push_blank(&mut list_items);
    list_items_push_focus(&mut list_items, "Note", "", false);
    list_items_push_focus(
        &mut list_items,
        "  custom_settings",
        format!(
            "Describe in '{}/edge_app_custom_settings.json' if needed",
            App::config_dir()
        )
        .as_str(),
        false,
    );

    let comment = ConfigKey::from(app.config_key_focus).note();
    list_items_push_focus(&mut list_items, "  Comment", comment, false);

    List::new(list_items)
        .block(normal_block(" EdgeApp Configuration "))
        .render(area, buf);

    Ok(())
}

pub fn draw_result_state(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    if let Some(config_result) = app.config_result.as_ref() {
        match config_result {
            Ok(s) => {
                let block = normal_block("Configuration Result");
                let root = json::parse(s).unwrap();

                Paragraph::new(json::stringify_pretty(root, 4))
                    .block(block)
                    .render(area, buf);
            }
            Err(e) => {
                let block = normal_block("Configuration Error");
                Paragraph::new(e.to_string()).block(block).render(area, buf);
            }
        }
    }
    Ok(())
}

pub fn draw(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    let current_screen = app.current_screen();
    match current_screen {
        DMScreen::EdgeApp(DMScreenState::Initial) => draw_default_state(area, buf, app)?,
        DMScreen::EdgeApp(DMScreenState::Configuring) => draw_configure_state(area, buf, app)?,
        DMScreen::EdgeApp(DMScreenState::Completed) => draw_result_state(area, buf, app)?,
        _ => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use error_stack::Report;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    fn test_draw_configure_and_result_states() {
        // Build an App via the public constructor
        let mut app = crate::app::App::new(crate::app::AppConfig { broker: "b" }).unwrap();

        // Prepare drawing area and buffer
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);

        // draw_configure_state should succeed with a default app
        assert!(draw_configure_state(area, &mut buf, &app).is_ok());

        // draw_result_state with an Ok config_result should succeed
        app.config_result = Some(Ok("{\"k\":1}".to_string()));
        assert!(draw_result_state(area, &mut buf, &app).is_ok());

        // draw_result_state with an Err config_result should also succeed
        app.config_result = Some(Err(Report::new(crate::error::DMError::InvalidData)));
        assert!(draw_result_state(area, &mut buf, &app).is_ok());
    }

    #[test]
    fn test_draw_render_paths_no_panics() {
        let app = crate::app::App::new(crate::app::AppConfig { broker: "b" }).unwrap();
        let area = Rect::new(0, 0, 50, 16);
        let mut buf = Buffer::empty(area);

        // call draw_configure_state and draw_result_state to exercise rendering code paths
        assert!(draw_configure_state(area, &mut buf, &app).is_ok());

        // result state when no config_result is set should be Ok (no-op)
        assert!(draw_result_state(area, &mut buf, &app).is_ok());
    }
}
