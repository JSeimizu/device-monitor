#[allow(unused)]
use {
    super::{list_items_push, list_items_push_focus, list_items_push_text_focus},
    crate::{
        app::{App, ui::focus_block, ui::normal_block},
        error::DMError,
    },
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

pub fn draw(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    let mut instances = vec![];
    // Get current deployed edge app
    if let Some(deployment_status) = app.mqtt_ctrl().deployment_status() {
        for id in deployment_status.instances().keys() {
            instances.push(id.uuid().to_owned());
        }
    }

    for instance in instances {
        if let Some(edge_app) = app.mqtt_ctrl().edge_app().get(&instance) {
            let title = format!("Edge App: {}", edge_app.id());
            let outer_block = focus_block(&title).borders(Borders::NONE);

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
                    list_items_push_text_focus(
                        &mut list_items,
                        &format!("process_state: {}", process_state),
                        false,
                    );
                }

                // Log Level
                if let Some(log_level) = common_settings.log_level() {
                    list_items_push_text_focus(
                        &mut list_items,
                        &format!("log_level: {}", log_level),
                        false,
                    );
                }

                // Inference Settings
                if let Some(inference_settings) = common_settings.inference_settings() {
                    list_items_push_text_focus(&mut list_items, "inference_settings", false);
                    if let Some(number_of_iterations) = inference_settings.number_of_iterations() {
                        list_items_push_text_focus(
                            &mut list_items,
                            &format!("  number_of_iterations: {}", number_of_iterations),
                            false,
                        );
                    }
                }

                // PQ Settings
                if let Some(pq_settings) = common_settings.pq_settings() {
                    list_items_push_text_focus(&mut list_items, "pq_settings", false);
                    if let Some(camera_image_size) = pq_settings.camera_image_size() {
                        list_items_push_text_focus(
                            &mut list_items,
                            &format!("  camera_image_size: {}", camera_image_size),
                            false,
                        );
                    }

                    if let Some(frame_rate) = pq_settings.frame_rate() {
                        list_items_push_text_focus(
                            &mut list_items,
                            &format!("  frame_rate: {}", frame_rate),
                            false,
                        );
                    }

                    if let Some(digital_zoom) = pq_settings.digital_zoom() {
                        list_items_push_text_focus(
                            &mut list_items,
                            &format!("  digital_zoom: {}", digital_zoom),
                            false,
                        );
                    }

                    if let Some(camera_image_flip) = pq_settings.camera_image_flip() {
                        list_items_push_text_focus(
                            &mut list_items,
                            &format!("  camera_image_flip: {}", camera_image_flip),
                            false,
                        );
                    }

                    // Exposure Mode
                    if let Some(exposure_mode) = pq_settings.exposure_mode() {
                        let mode = match exposure_mode {
                            0 => "auto",
                            1 => "manual",
                            _ => "invalid",
                        };

                        list_items_push_text_focus(
                            &mut list_items,
                            &format!("  exposure_mode: {mode} (exposure_mode)",),
                            false,
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
                                    list_items_push_text_focus(
                                        &mut list_items,
                                        &format!("    max_exposure_time: {}", max_exposure_time),
                                        false,
                                    );
                                }

                                if let Some(min_exposure_time) = auto_exposure.min_exposure_time() {
                                    list_items_push_text_focus(
                                        &mut list_items,
                                        &format!("    min_exposure_time: {}", min_exposure_time),
                                        false,
                                    );
                                }

                                if let Some(max_gain) = auto_exposure.max_gain() {
                                    list_items_push_text_focus(
                                        &mut list_items,
                                        &format!("    max_gain: {}", max_gain),
                                        false,
                                    );
                                }

                                if let Some(convergence_speed) = auto_exposure.convergence_speed() {
                                    list_items_push_text_focus(
                                        &mut list_items,
                                        &format!("    convergence_speed: {}", convergence_speed),
                                        false,
                                    );
                                }
                            }

                            // EV compensation
                            if let Some(ev_compensation) = pq_settings.ev_compensation() {
                                list_items_push_text_focus(
                                    &mut list_items,
                                    &format!("  ev_compensation: {}", ev_compensation),
                                    false,
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
                                list_items_push_text_focus(
                                    &mut list_items,
                                    &format!(
                                        "  ae_anti_flicker_mode: {mode} (ae_anti_flicker_mode)"
                                    ),
                                    false,
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
                                    list_items_push_text_focus(
                                        &mut list_items,
                                        &format!("    exposure_time: {}", exposure_time),
                                        false,
                                    );
                                }

                                if let Some(gain) = manual_exposure.gain() {
                                    list_items_push_text_focus(
                                        &mut list_items,
                                        &format!("    gain: {}", gain),
                                        false,
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
                        list_items_push_text_focus(
                            &mut list_items,
                            &format!("  white_balance_mode: {mode} (white_balance_mode)"),
                            false,
                        );

                        // If white balance mode is auto, show auto white balance settings
                        if white_balance_mode == 0 {
                            if let Some(auto_white_balance) = pq_settings.auto_white_balance() {
                                list_items_push_text_focus(
                                    &mut list_items,
                                    &format!("  auto_white_balance"),
                                    false,
                                );

                                if let Some(convergence_speed) =
                                    auto_white_balance.convergence_speed()
                                {
                                    list_items_push_text_focus(
                                        &mut list_items,
                                        &format!("    convergence_speed: {}", convergence_speed),
                                        false,
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

                                    list_items_push_text_focus(
                                        &mut list_items,
                                        &format!(
                                            "    color_temperature: {}({})",
                                            color_temp, color_temperature
                                        ),
                                        false,
                                    );
                                }
                            }
                        }
                    }

                    if let Some(image_cropping) = pq_settings.image_cropping() {
                        list_items_push_text_focus(
                            &mut list_items,
                            &format!("  image_cropping: {}", image_cropping),
                            false,
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
                        list_items_push_text_focus(
                            &mut list_items,
                            &format!(
                                "  image_rotation: {} ({})",
                                image_rotation_str, image_rotation
                            ),
                            false,
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

                            list_items_push_text_focus(
                                &mut list_items,
                                &format!("    method: {} ({})", m, method),
                                false,
                            );
                        }

                        if let Some(storage_name) = metadata.storage_name() {
                            list_items_push_text_focus(
                                &mut list_items,
                                &format!("    storage_name: {}", storage_name),
                                false,
                            );
                        }

                        if let Some(endpoint) = metadata.endpoint() {
                            list_items_push_text_focus(
                                &mut list_items,
                                &format!("    endpoint: {}", endpoint),
                                false,
                            );
                        }

                        if let Some(path) = metadata.path() {
                            list_items_push_text_focus(
                                &mut list_items,
                                &format!("    path: {}", path),
                                false,
                            );
                        }

                        if let Some(enabled) = metadata.enabled() {
                            list_items_push_text_focus(
                                &mut list_items,
                                &format!("    enabled: {}", enabled),
                                false,
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

                            list_items_push_text_focus(
                                &mut list_items,
                                &format!("    method: {} ({})", m, method),
                                false,
                            );
                        }

                        if let Some(storage_name) = input_tensor.storage_name() {
                            list_items_push_text_focus(
                                &mut list_items,
                                &format!("    storage_name: {}", storage_name),
                                false,
                            );
                        }

                        if let Some(endpoint) = input_tensor.endpoint() {
                            list_items_push_text_focus(
                                &mut list_items,
                                &format!("    endpoint: {}", endpoint),
                                false,
                            );
                        }

                        if let Some(path) = input_tensor.path() {
                            list_items_push_text_focus(
                                &mut list_items,
                                &format!("    path: {}", path),
                                false,
                            );
                        }

                        if let Some(enabled) = input_tensor.enabled() {
                            list_items_push_text_focus(
                                &mut list_items,
                                &format!("    enabled: {}", enabled),
                                false,
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
                        list_items_push_text_focus(
                            &mut list_items,
                            &format!("  codec: {} ({})", format_str, format),
                            false,
                        );
                    }
                }

                // Number of inference per message
                if let Some(number_of_inference_per_message) =
                    common_settings.number_of_inference_per_message()
                {
                    list_items_push_text_focus(
                        &mut list_items,
                        &format!(
                            "number_of_inference_per_message: {}",
                            number_of_inference_per_message
                        ),
                        false,
                    );
                }

                // upload_interval
                if let Some(upload_interval) = common_settings.upload_interval() {
                    list_items_push_text_focus(
                        &mut list_items,
                        &format!("upload_interval: {}", upload_interval),
                        false,
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
                    list_items_push_text_focus(
                        &mut list_items,
                        &format!("  req_id: {}", req_info.req_id()),
                        false,
                    );
                }

                if let Some(res_info) = edge_app.module().res_info() {
                    list_items_push_text_focus(&mut list_items, "res_info", false);
                    list_items_push_text_focus(
                        &mut list_items,
                        &format!("  res_id: {}", res_info.res_id()),
                        false,
                    );

                    list_items_push_text_focus(
                        &mut list_items,
                        &format!("  code: {}", res_info.code_str()),
                        false,
                    );
                    list_items_push_text_focus(
                        &mut list_items,
                        &format!("  detail_msg: {}", res_info.detail_msg()),
                        false,
                    );
                }

                let req_res_block = normal_block("Req/Res Info");
                List::new(list_items)
                    .block(req_res_block)
                    .render(right_chunks[0], buf);
            }

            // Custom Settings
            {
                let custom_settings_block = normal_block("Custom Settings");
                let mut list_items = Vec::<ListItem>::new();
                List::new(list_items)
                    .block(custom_settings_block)
                    .render(right_chunks[1], buf);
            }

            // Only render the first instance
            break;
        }
    }
    Ok(())
}
