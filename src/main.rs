mod app;
mod error;
mod mqtt_ctrl;

use std::io::Stderr;
#[allow(unused)]
use {
    app::{App, AppConfig},
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
        io,
        time::{Duration, Instant},
    },
};

#[derive(Parser)]
#[command(author, version, about, long_about=None)]
pub struct Cli {
    /// MQTT broker address
    #[arg(short, long, default_value_t=String::from("localhost:1883"))]
    broker: String,

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

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<(), DMError> {
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

        if app.should_exit() {
            break;
        }

        let draw = Instant::now();
        app.update()?;
        jinfo!(
            event = "TIME_MEASURE",
            app_update_time = format!("{}ms", draw.elapsed().as_millis())
        );

        terminal
            .draw(|frame| app.draw(frame))
            .map_err(|e| Report::new(DMError::IOError).attach_printable(e))?;

        let draw = Instant::now();
        app.handle_events()?;
        jinfo!(
            event = "TIME_MEASURE",
            handle_events_time = format!("{}ms", draw.elapsed().as_millis())
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

    let mut app = App::new(AppConfig {
        broker: &cli.broker,
    })?;
    let app_result = run_app(&mut terminal, &mut app);
    dm_teardown(terminal)?;

    app_result
}
