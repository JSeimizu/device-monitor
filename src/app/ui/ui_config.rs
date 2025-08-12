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
    super::centered_rect,
    super::*,
    crate::{
        app::{App, ConfigKey, DMScreen, MainWindowFocus},
        error::{DMError, DMErrorExt},
        mqtt_ctrl::MqttCtrl,
    },
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    error_stack::{Report, Result},
    jlogger_tracing::{JloggerBuilder, LevelFilter, LogTimeFormat, jdebug, jerror, jinfo},
    json::{JsonValue, object::Object},
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

fn draw_wireless_settings(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
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
    list_items_push_focus(
        &mut list_items,
        "station_mode_ssid",
        &value(ConfigKey::StaSsid),
        focus(ConfigKey::StaSsid),
    );

    list_items_push_focus(
        &mut list_items,
        "station_mode_password",
        &value(ConfigKey::StaPassword),
        focus(ConfigKey::StaPassword),
    );

    list_items_push_focus(
        &mut list_items,
        "station_mode_encryption",
        &value(ConfigKey::StaEncryption),
        focus(ConfigKey::StaEncryption),
    );

    list_items_push_blank(&mut list_items);
    list_items_push_focus(&mut list_items, "Note", "", false);
    let comment = ConfigKey::from(app.config_key_focus).note();
    list_items_push_focus(&mut list_items, "  Comment", comment, false);

    List::new(list_items)
        .block(normal_block(" Configuration "))
        .render(area, buf);
    Ok(())
}

fn draw_network_settings(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    let focus = |config_key| ConfigKey::from(app.config_key_focus) == config_key;

    let value = |config_key| {
        if app.config_key_editable && focus(config_key) {
            format!("{}|", &app.config_keys[usize::from(config_key)])
        } else {
            app.config_keys[usize::from(config_key)].to_string()
        }
    };

    let mut list_items = Vec::<ListItem>::new();

    list_items_push_focus(
        &mut list_items,
        "ip_method",
        &value(ConfigKey::IpMethod),
        focus(ConfigKey::IpMethod),
    );

    list_items_push_focus(
        &mut list_items,
        "ntp_url",
        &value(ConfigKey::NtpUrl),
        focus(ConfigKey::NtpUrl),
    );

    list_items_push_focus(
        &mut list_items,
        "static_ipv4_ip",
        &value(ConfigKey::StaticIpv4Ip),
        focus(ConfigKey::StaticIpv4Ip),
    );

    list_items_push_focus(
        &mut list_items,
        "static_ipv4_subnet_mask",
        &value(ConfigKey::StaticIpv4SubnetMask),
        focus(ConfigKey::StaticIpv4SubnetMask),
    );

    list_items_push_focus(
        &mut list_items,
        "static_ipv4_gateway",
        &value(ConfigKey::StaticIpv4Gateway),
        focus(ConfigKey::StaticIpv4Gateway),
    );

    list_items_push_focus(
        &mut list_items,
        "static_ipv4_dns",
        &value(ConfigKey::StaticIpv4Dns),
        focus(ConfigKey::StaticIpv4Dns),
    );

    list_items_push_focus(
        &mut list_items,
        "static_ipv6_ip",
        &value(ConfigKey::StaticIpv6Ip),
        focus(ConfigKey::StaticIpv6Ip),
    );

    list_items_push_focus(
        &mut list_items,
        "static_ipv6_subnet_mask",
        &value(ConfigKey::StaticIpv6SubnetMask),
        focus(ConfigKey::StaticIpv6SubnetMask),
    );

    list_items_push_focus(
        &mut list_items,
        "static_ipv6_gateway",
        &value(ConfigKey::StaticIpv6Gateway),
        focus(ConfigKey::StaticIpv6Gateway),
    );

    list_items_push_focus(
        &mut list_items,
        "static_ipv6_dns",
        &value(ConfigKey::StaticIpv6Dns),
        focus(ConfigKey::StaticIpv6Dns),
    );

    list_items_push_focus(
        &mut list_items,
        "proxy_url",
        &value(ConfigKey::ProxyUrl),
        focus(ConfigKey::ProxyUrl),
    );

    list_items_push_focus(
        &mut list_items,
        "proxy_port",
        &value(ConfigKey::ProxyPort),
        focus(ConfigKey::ProxyPort),
    );

    list_items_push_focus(
        &mut list_items,
        "proxy_user_name",
        &value(ConfigKey::ProxyUserName),
        focus(ConfigKey::ProxyUserName),
    );

    list_items_push_focus(
        &mut list_items,
        "proxy_password",
        &value(ConfigKey::ProxyPassword),
        focus(ConfigKey::ProxyPassword),
    );

    list_items_push_blank(&mut list_items);
    list_items_push_focus(&mut list_items, "Note", "", false);

    let comment = ConfigKey::from(app.config_key_focus).note();
    list_items_push_focus(&mut list_items, "  Comment", comment, false);

    List::new(list_items)
        .block(normal_block(" Configuration "))
        .render(area, buf);
    Ok(())
}

fn draw_agent_state(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    let focus = |config_key| ConfigKey::from(app.config_key_focus) == config_key;

    let value = |config_key| {
        if app.config_key_editable && focus(config_key) {
            format!("{}|", &app.config_keys[usize::from(config_key)])
        } else {
            app.config_keys[usize::from(config_key)].to_string()
        }
    };

    let mut list_items = Vec::<ListItem>::new();
    list_items_push_focus(
        &mut list_items,
        "report_status_interval_min",
        &value(ConfigKey::ReportStatusIntervalMin),
        focus(ConfigKey::ReportStatusIntervalMin),
    );

    list_items_push_focus(
        &mut list_items,
        "report_status_interval_max",
        &value(ConfigKey::ReportStatusIntervalMax),
        focus(ConfigKey::ReportStatusIntervalMax),
    );

    List::new(list_items)
        .block(normal_block(" Configuration "))
        .render(area, buf);
    Ok(())
}

fn draw_system_settings(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    let focus = |config_key| ConfigKey::from(app.config_key_focus) == config_key;

    let value = |config_key| {
        if app.config_key_editable && focus(config_key) {
            format!("{}|", &app.config_keys[usize::from(config_key)])
        } else {
            app.config_keys[usize::from(config_key)].to_string()
        }
    };

    let mut list_items = Vec::<ListItem>::new();
    list_items_push_focus(
        &mut list_items,
        "led_enabled",
        &value(ConfigKey::LedEnabled),
        focus(ConfigKey::LedEnabled),
    );

    list_items_push_focus(
        &mut list_items,
        "temperature_update_interval",
        &value(ConfigKey::TemperatureUpdateInterval),
        focus(ConfigKey::TemperatureUpdateInterval),
    );

    // all
    {
        list_items_push_focus(
            &mut list_items,
            "log.all.level",
            &value(ConfigKey::AllLogSettingLevel),
            focus(ConfigKey::AllLogSettingLevel),
        );

        list_items_push_focus(
            &mut list_items,
            "log.all.destination",
            &value(ConfigKey::AllLogSettingDestination),
            focus(ConfigKey::AllLogSettingDestination),
        );

        list_items_push_focus(
            &mut list_items,
            "log.all.storage_name",
            &value(ConfigKey::AllLogSettingStorageName),
            focus(ConfigKey::AllLogSettingStorageName),
        );

        list_items_push_focus(
            &mut list_items,
            "log.all.path",
            &value(ConfigKey::AllLogSettingPath),
            focus(ConfigKey::AllLogSettingPath),
        );
    }

    // main
    {
        list_items_push_focus(
            &mut list_items,
            "log.main.level",
            &value(ConfigKey::MainLogSettingLevel),
            focus(ConfigKey::MainLogSettingLevel),
        );

        list_items_push_focus(
            &mut list_items,
            "log.main.destination",
            &value(ConfigKey::MainLogSettingDestination),
            focus(ConfigKey::MainLogSettingDestination),
        );

        list_items_push_focus(
            &mut list_items,
            "log.main.storage_name",
            &value(ConfigKey::MainLogSettingStorageName),
            focus(ConfigKey::MainLogSettingStorageName),
        );

        list_items_push_focus(
            &mut list_items,
            "log.main.path",
            &value(ConfigKey::MainLogSettingPath),
            focus(ConfigKey::MainLogSettingPath),
        );
    }

    // sensor
    {
        list_items_push_focus(
            &mut list_items,
            "log.sensor.level",
            &value(ConfigKey::SensorLogSettingLevel),
            focus(ConfigKey::SensorLogSettingLevel),
        );

        list_items_push_focus(
            &mut list_items,
            "log.sensor.destination",
            &value(ConfigKey::SensorLogSettingDestination),
            focus(ConfigKey::SensorLogSettingDestination),
        );

        list_items_push_focus(
            &mut list_items,
            "log.sensor.storage_name",
            &value(ConfigKey::SensorLogSettingStorageName),
            focus(ConfigKey::SensorLogSettingStorageName),
        );

        list_items_push_focus(
            &mut list_items,
            "log.sensor.path",
            &value(ConfigKey::SensorLogSettingPath),
            focus(ConfigKey::SensorLogSettingPath),
        );
    }

    // companion_fw
    {
        list_items_push_focus(
            &mut list_items,
            "log.fw.level",
            &value(ConfigKey::CompanionFwLogSettingLevel),
            focus(ConfigKey::CompanionFwLogSettingLevel),
        );

        list_items_push_focus(
            &mut list_items,
            "log.fw.destination",
            &value(ConfigKey::CompanionFwLogSettingDestination),
            focus(ConfigKey::CompanionFwLogSettingDestination),
        );

        list_items_push_focus(
            &mut list_items,
            "log.fw.storage_name",
            &value(ConfigKey::CompanionFwLogSettingStorageName),
            focus(ConfigKey::CompanionFwLogSettingStorageName),
        );

        list_items_push_focus(
            &mut list_items,
            "log.fw.path",
            &value(ConfigKey::CompanionFwLogSettingPath),
            focus(ConfigKey::CompanionFwLogSettingPath),
        );
    }

    // companion_app
    {
        list_items_push_focus(
            &mut list_items,
            "log.app.level",
            &value(ConfigKey::CompanionAppLogSettingLevel),
            focus(ConfigKey::CompanionAppLogSettingLevel),
        );

        list_items_push_focus(
            &mut list_items,
            "log.app.destination",
            &value(ConfigKey::CompanionAppLogSettingDestination),
            focus(ConfigKey::CompanionAppLogSettingDestination),
        );

        list_items_push_focus(
            &mut list_items,
            "log.app.storage_name",
            &value(ConfigKey::CompanionAppLogSettingStorageName),
            focus(ConfigKey::CompanionAppLogSettingStorageName),
        );

        list_items_push_focus(
            &mut list_items,
            "log.app.path",
            &value(ConfigKey::CompanionAppLogSettingPath),
            focus(ConfigKey::CompanionAppLogSettingPath),
        );
    }

    list_items_push_blank(&mut list_items);
    list_items_push_focus(&mut list_items, "Note", "", false);

    let comment = ConfigKey::from(app.config_key_focus).note();
    list_items_push_focus(&mut list_items, "  Comment", comment, false);

    List::new(list_items)
        .block(normal_block(" Configuration "))
        .render(area, buf);
    Ok(())
}

pub fn draw(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    if let Some(result) = app.config_result.as_ref() {
        match result {
            Ok(s) => {
                let block = normal_block("Configuration Result");
                let root = json::parse(s).unwrap();

                if let Some((k, v)) = root.entries().next() {
                    // Json entry in DTDL for SystemApp is stored as json string
                    // transfer it to normal json object for a pretty view.
                    if let JsonValue::String(s) = v {
                        let json = json::parse(s).unwrap();
                        let mut root = Object::new();
                        root.insert(k, json);
                        Paragraph::new(json::stringify_pretty(root, 4))
                            .block(block)
                            .render(area, buf);
                    } else {
                        Paragraph::new(s.to_owned()).block(block).render(area, buf);
                    }
                }
            }
            Err(e) => {
                let block = normal_block("Configuration Error");
                let s = e.error_str().unwrap_or_else(|| e.to_string());
                Paragraph::new(s).block(block).render(area, buf);
            }
        }
        Ok(())
    } else {
        match app.main_window_focus() {
            MainWindowFocus::AgentState => draw_agent_state(area, buf, app),
            MainWindowFocus::SystemSettings => draw_system_settings(area, buf, app),
            MainWindowFocus::NetworkSettings => draw_network_settings(area, buf, app),
            MainWindowFocus::WirelessSettings => draw_wireless_settings(area, buf, app),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    fn test_draw_variants() {
        // Create an App with default config via App::new
        let mut app = crate::app::App::new(crate::app::AppConfig { broker: "b" }).unwrap();

        // Prepare drawing area and buffer
        let area = Rect::new(0, 0, 40, 20);
        let mut buf = Buffer::empty(area);

        // Call the individual draw functions (should return Ok(()))
        assert!(draw_wireless_settings(area, &mut buf, &app).is_ok());
        assert!(draw_network_settings(area, &mut buf, &app).is_ok());
        assert!(draw_agent_state(area, &mut buf, &app).is_ok());
        assert!(draw_system_settings(area, &mut buf, &app).is_ok());

        // Test the top-level draw when config_result is present (Ok)
        app.config_result = Some(Ok(
            r#"{"desiredDeviceConfig":"{\"configuration/$agent/report-status-interval-min\":5}"}"#
                .to_string(),
        ));
        assert!(draw(area, &mut buf, &app).is_ok());

        // Test the top-level draw when config_result is an Err
        app.config_result = Some(Err(Report::new(crate::error::DMError::InvalidData)));
        assert!(draw(area, &mut buf, &app).is_ok());
    }
}
