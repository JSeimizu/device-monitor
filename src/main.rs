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

mod ai_model;
mod app;
mod azurite;
mod error;
mod mqtt_ctrl;
mod ota;

#[allow(unused)]
use {
    app::{AppConfig, draw, handle_events, init_global_app, should_exit, update},
    azurite::init_global_azurite_storage,
    clap::Parser,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    error::DMError,
    error_stack::{Report, Result},
    jlogger_tracing::{JloggerBuilder, LevelFilter, LogTimeFormat, jdebug, jerror, jinfo},
    mqtt_ctrl::MqttCtrl,
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
    serde_derive::{Deserialize, Serialize},
    std::{
        collections::HashMap,
        io::{self, Stderr},
        time::{Duration, Instant},
    },
};

#[derive(Parser)]
#[command(author, version, about, long_about=None)]
pub struct Cli {
    /// MQTT broker address
    #[arg(short, long, default_value_t=String::from("localhost:1883"))]
    broker: String,

    /// Azurite url
    #[arg(short, long, default_value_t=String::from("https://127.0.1:10000"))]
    azurite_url: String,

    /// Log file
    #[arg(short, long)]
    log: Option<String>,

    /// Verbose
    #[arg(short, long, action=clap::ArgAction::Count)]
    verbose: u8,
}

fn dm_setup() -> Result<Terminal<CrosstermBackend<Stderr>>, DMError> {
    // Initial terminal
    enable_raw_mode().map_err(|e| Report::new(DMError::IOError).attach_printable(e))?;

    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)
        .map_err(|e| Report::new(DMError::IOError).attach_printable(e))?;

    let backend = CrosstermBackend::new(stderr);
    let mut terminal =
        Terminal::new(backend).map_err(|e| Report::new(DMError::IOError).attach_printable(e))?;
    let _ = terminal.clear();
    Ok(terminal)
}

fn dm_teardown(mut terminal: Terminal<CrosstermBackend<Stderr>>) -> Result<(), DMError> {
    // Restore terminal
    disable_raw_mode().map_err(|e| Report::new(DMError::IOError).attach_printable(e))?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .map_err(|e| Report::new(DMError::IOError).attach_printable(e))?;

    terminal
        .show_cursor()
        .map_err(|e| Report::new(DMError::IOError).attach_printable(e))
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>) -> Result<(), DMError> {
    jdebug!(func = "run_app", line = line!(), note = "Main loop");
    let mut draw_now = Instant::now();
    let mut draw_old;

    loop {
        draw_old = draw_now;
        draw_now = Instant::now();

        jinfo!(
            event = "TIME_MEASURE",
            main_loop_time = format!("{}ms", (draw_now - draw_old).as_millis())
        );

        if should_exit() {
            break;
        }

        let draw_time = Instant::now();
        update()?;
        jinfo!(
            event = "TIME_MEASURE",
            app_update_time = format!("{}ms", draw_time.elapsed().as_millis())
        );

        terminal
            .draw(draw)
            .map_err(|e| Report::new(DMError::IOError).attach_printable(e))?;

        let events_time = Instant::now();
        handle_events()?;
        jinfo!(
            event = "TIME_MEASURE",
            handle_events_time = format!("{}ms", events_time.elapsed().as_millis())
        );
    }

    Ok(())
}

fn main() -> Result<(), DMError> {
    let cli = Cli::parse();

    let level = match cli.verbose {
        1 => LevelFilter::DEBUG,
        2 => LevelFilter::TRACE,
        _ => LevelFilter::INFO,
    };

    if let Some(log_file) = cli.log.as_deref() {
        JloggerBuilder::new()
            .max_level(level)
            .log_file(Some((log_file, false)))
            .log_console(false)
            .log_time(LogTimeFormat::TimeLocal)
            .build();
    }

    jdebug!(func = "main", line = line!(), note = "Starting app");
    let mut terminal = dm_setup()?;

    // Initialize global MqttCtrl first, then global AzuriteStorage, then global App
    mqtt_ctrl::init_global_mqtt_ctrl(&cli.broker)?;
    init_global_azurite_storage(&cli.azurite_url)?;
    init_global_app(AppConfig {
        broker: &cli.broker,
    })?;

    let app_result = run_app(&mut terminal);
    dm_teardown(terminal)?;

    app_result
}

#[cfg(test)]
#[ctor::ctor]
fn test_init() {
    // Initialize logger for tests
    // Set log_console to true to see logs in the console during tests
    JloggerBuilder::new()
        .max_level(LevelFilter::DEBUG)
        .log_console(false)
        .build();
}
