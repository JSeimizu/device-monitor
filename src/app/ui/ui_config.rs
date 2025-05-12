use crate::app::{ConfigKey, MainWindowFocus};
#[allow(unused)]
use {
    super::centered_rect,
    super::*,
    crate::{
        app::{App, DMScreen},
        error::{DMError, DMErrorExt},
        mqtt_ctrl::MqttCtrl,
    },
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

fn draw_agent_state(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    let focus = |config_key| ConfigKey::from(app.config_key_focus) == config_key;

    let value = |config_key| {
        if app.config_key_editable && focus(config_key) {
            format!("{}|", &app.config_keys[usize::from(config_key)])
        } else {
            format!("{}", &app.config_keys[usize::from(config_key)])
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
            format!("{}", &app.config_keys[usize::from(config_key)])
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
                Paragraph::new(s.to_owned()).block(block).render(area, buf);
            }
            Err(e) => {
                let block = normal_block("Configuration Error");
                let s = e.error_str().unwrap();
                Paragraph::new(s).block(block).render(area, buf);
            }
        }
        Ok(())
    } else {
        match app.main_window_focus() {
            MainWindowFocus::AgentState => draw_agent_state(area, buf, app),
            MainWindowFocus::SystemSettings => draw_system_settings(area, buf, app),
            _ => Ok(()),
        }
    }
}
