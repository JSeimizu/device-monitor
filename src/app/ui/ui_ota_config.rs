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
    super::{
        list_items_push, list_items_push_blank, list_items_push_focus, list_items_push_text_focus,
    },
    crate::{
        app::{App, ConfigKey, DMScreen, DMScreenState, ui::focus_block, ui::normal_block},
        error::{DMError, DMErrorExt},
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
        widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Widget},
    },
};

pub fn draw_initial(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    let block = normal_block(" OTA Configuration ").border_type(BorderType::Rounded);

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
    let comment = ConfigKey::from(app.config_key_focus).note();
    list_items_push_focus(&mut list_items, "  Comment", comment, false);

    List::new(list_items).block(block).render(area, buf);

    Ok(())
}

pub fn draw_configuring(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    Ok(())
}

pub fn draw_completed(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    if let Some(config_result) = app.config_result.as_ref() {
        match config_result {
            Ok(s) => {
                let block =
                    normal_block(" OTA Configuration Result").border_type(BorderType::Rounded);

                let root = json::parse(s).unwrap();

                let mut root_new = Object::new();

                match root {
                    JsonValue::Object(obj) => {
                        if let Some(content) =
                            obj.get("configuration/$system/PRIVATE_deploy_firmware")
                        {
                            match content {
                                JsonValue::String(s) => {
                                    if let Ok(obj) = json::parse(s) {
                                        root_new.insert(
                                            "configuration/$system/PRIVATE_deploy_firmware",
                                            obj,
                                        );
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }

                Paragraph::new(json::stringify_pretty(root_new, 4))
                    .block(block)
                    .render(area, buf);
            }

            Err(e) => {
                let block = normal_block("OTA Configuration Error");
                let s = e.error_str().unwrap_or_else(|| e.to_string());
                Paragraph::new(s).block(block).render(area, buf);
            }
        }
    }
    Ok(())
}

pub fn draw(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    let current_screen = app.current_screen();
    match current_screen {
        DMScreen::OtaConfig(DMScreenState::Initial) => draw_initial(area, buf, app)?,
        DMScreen::OtaConfig(DMScreenState::Configuring) => draw_configuring(area, buf, app)?,
        DMScreen::OtaConfig(DMScreenState::Completed) => draw_completed(area, buf, app)?,
        _ => {}
    }

    Ok(())
}
