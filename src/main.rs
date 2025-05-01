mod error;

use std::time::{Duration, Instant};

use jlogger_tracing::jerror;
use rumqttc::Connection;
#[allow(unused)]
use {
    clap::Parser,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    error::DMError,
    error_stack::{Report, Result},
    jlogger_tracing::{JloggerBuilder, LevelFilter, LogTimeFormat, jdebug, jinfo},
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
    regex::Regex,
    rumqttc::{Client, MqttOptions, QoS, matches},
    serde_derive::{Deserialize, Serialize},
    std::{collections::HashMap, io, time},
};

#[derive(Parser)]
#[command(author, version, about, long_about=None)]
pub struct Cli {
    #[arg(short, long)]
    broker: Option<String>,

    #[arg(short = 't', long)]
    topic_file: Option<String>,

    #[arg(short, long, action=clap::ArgAction::Count)]
    verbose: u8,

    #[arg(short = 'H', long, default_value_t = String::from("127.0.0.1:8080"))]
    http_server_url: String,
}

#[derive(Debug, Default)]
pub enum CurrentScreen {
    #[default]
    Main,
    Editing,
    Exiting,
}

#[derive(Debug, Default, PartialEq, PartialOrd)]
pub enum CurrentlyEditing {
    #[default]
    None,
    Key,
    Value,
}

pub struct MqttCtrl {
    client: Client,
    conn: Connection,
    device_connected: bool,
    last_connected: time::Instant,
}

impl MqttCtrl {
    pub fn new(url: &str, port: u16) -> Result<Self, DMError> {
        let mut mqtt_options = MqttOptions::new("device-monitor", url, port);
        mqtt_options.set_keep_alive(Duration::from_secs(60));

        jdebug!(
            func = "MqttCtrl::new()",
            line = line!(),
            url = url,
            port = port
        );
        let (client, conn) = Client::new(mqtt_options, 10);

        client
            .subscribe("#", QoS::AtLeastOnce)
            .map_err(|e| Report::new(DMError::IOError).attach_printable(e))?;

        jdebug!(
            func = "MqttCtrl::new()",
            line = line!(),
            note = "All topic subscribed"
        );

        Ok(Self {
            client,
            conn,
            device_connected: false,
            last_connected: time::Instant::now(),
        })
    }

    pub fn is_device_connected(&self) -> bool {
        self.device_connected
    }

    fn process_device_connect_req(&mut self, topic: &str, payload: &str) -> Result<bool, DMError> {
        let re = Regex::new(r"^v1\/devices\/([^\/]+)\/attributes\/request\/(\d+)$")
            .map_err(|_| DMError::InvalidData)?;

        if let Some(caps) = re.captures(topic) {
            let who = &caps[1];
            let req_id: u32 = caps[2].parse().unwrap();

            jinfo!(
                func = "process_device_connect_req",
                topic = topic,
                payload = payload
            );

            self.client
                .publish(
                    &format!("v1/devices/{who}/attributes/response/{req_id}"),
                    QoS::AtLeastOnce,
                    false,
                    payload,
                )
                .map_err(|_| Report::new(DMError::IOError))?;

            self.device_connected = true;
            self.last_connected = time::Instant::now();

            return Ok(true);
        }

        Ok(false)
    }

    pub fn process_agent_request(
        &mut self,
        topic: &str,
        payload: &str,
    ) -> Result<HashMap<String, String>, DMError> {
        let mut result = HashMap::new();

        let processed = self.process_device_connect_req(topic, payload)?;
        if processed {
            return Ok(result);
        }

        result.insert(topic.to_owned(), payload.to_owned());
        Ok(result)
    }

    pub fn read(&mut self) -> Result<HashMap<String, String>, DMError> {
        let mut result = HashMap::new();
        jdebug!(func = "MqttCtrl::read()", line = line!());

        match self.conn.recv_timeout(Duration::from_millis(100)) {
            Ok(v) => match v {
                Ok(event) => match event {
                    rumqttc::Event::Incoming(i_event) => match i_event {
                        rumqttc::Packet::Publish(data) => {
                            jdebug!(func = "MqttCtrl::read()", line = line!(), note = "publish");
                            let topic = data.topic;
                            let payload = String::from_utf8(data.payload.to_vec())
                                .map_err(|e| Report::new(DMError::InvalidData))?;

                            result.extend(self.process_agent_request(&topic, &payload)?);
                        }
                        _ => {
                            jdebug!(func = "MqttCtrl::read()", line = line!(), note = "others");
                        }
                    },
                    rumqttc::Event::Outgoing(o_event) => {}
                },
                Err(e) => {
                    jdebug!(
                        func = "MqttCtrl::read()",
                        line = line!(),
                        error = format!("{e}")
                    );
                }
            },
            Err(_e) => {
                jdebug!(
                    func = "MqttCtrl::read()",
                    line = line!(),
                    error = format!("RecvError")
                );
            }
        }

        // If there is no messages from device for 5 mintues
        // device is considered to be disconnected.
        if self.last_connected.elapsed() > Duration::from_secs(5 * 60) {
            self.device_connected = false;
        }

        Ok(result)
    }
}

pub struct App {
    exit: bool,
    should_print_json: bool,
    mqtt_ctrl: MqttCtrl,
    key_input: Option<String>,
    value_input: Option<String>,
    pairs: HashMap<String, String>,
    current_screen: CurrentScreen,
    currently_editing: CurrentlyEditing,
}

impl App {
    pub fn new(cli: &Cli) -> Result<Self, DMError> {
        let broker = cli.broker.as_deref().unwrap_or("localhost:1883");
        let (broker_url, broker_port_str) = broker.split_once(':').unwrap();
        let broker_port = broker_port_str.parse().unwrap_or(1883);

        let mqtt_ctrl = MqttCtrl::new(broker_url, broker_port)?;

        Ok(Self {
            mqtt_ctrl,
            exit: false,
            should_print_json: false,
            key_input: None,
            value_input: None,
            pairs: HashMap::new(),
            current_screen: CurrentScreen::Main,
            currently_editing: CurrentlyEditing::None,
        })
    }

    pub fn save_key_value(&mut self) {
        self.pairs.insert(
            self.key_input.take().unwrap_or_default(),
            self.value_input.take().unwrap_or_default(),
        );
        self.currently_editing = CurrentlyEditing::None;
    }

    pub fn toggle_editing(&mut self) {
        let next = match self.currently_editing {
            CurrentlyEditing::Key => CurrentlyEditing::Value,
            CurrentlyEditing::Value => CurrentlyEditing::Key,
            CurrentlyEditing::None => CurrentlyEditing::None,
        };
        self.currently_editing = next;
    }

    pub fn print_json(&self) -> Result<(), DMError> {
        if self.should_print_json {
            let output = serde_json::to_string(&self.pairs)
                .map_err(|e| Report::new(DMError::InvalidData).attach_printable(e))?;
            println!("{}", output);
        }
        Ok(())
    }

    pub fn update(&mut self) -> Result<(), DMError> {
        self.pairs.extend(self.mqtt_ctrl.read()?);
        Ok(())
    }

    pub fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    pub fn handle_events(&mut self) -> Result<(), DMError> {
        let has_new_event = event::poll(Duration::from_millis(500))
            .map_err(|e| Report::new(DMError::IOError).attach_printable(e))?;

        if has_new_event {
            let event = event::read().map_err(|_| Report::new(DMError::IOError))?;
            match event {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    self.handle_key_event(key_event)
                }
                _ => {}
            }
        }

        Ok(())
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) {
        match self.current_screen {
            CurrentScreen::Main => match key_event.code {
                KeyCode::Char('e') => {
                    self.current_screen = CurrentScreen::Editing;
                    self.currently_editing = CurrentlyEditing::Key;
                }
                KeyCode::Char('q') => {
                    self.current_screen = CurrentScreen::Exiting;
                }
                _ => {}
            },
            CurrentScreen::Exiting => {
                match key_event.code {
                    KeyCode::Char('y') => {
                        self.should_print_json = true;
                    }
                    KeyCode::Char('n') => {
                        self.should_print_json = false;
                    }
                    _ => {}
                };
                self.exit = true;
            }
            CurrentScreen::Editing => match key_event.code {
                KeyCode::Enter => match self.currently_editing {
                    CurrentlyEditing::Key => {
                        self.currently_editing = CurrentlyEditing::Value;
                    }
                    CurrentlyEditing::Value => {
                        self.save_key_value();
                        self.current_screen = CurrentScreen::Main;
                    }
                    _ => {}
                },
                KeyCode::Backspace => match self.currently_editing {
                    CurrentlyEditing::Key => {
                        if let Some(input) = &mut self.key_input {
                            input.pop();
                        }
                    }
                    CurrentlyEditing::Value => {
                        if let Some(input) = &mut self.value_input {
                            input.pop();
                        }
                    }
                    _ => {}
                },
                KeyCode::Esc => {
                    self.current_screen = CurrentScreen::Main;
                    self.currently_editing = CurrentlyEditing::None;
                }
                KeyCode::Tab => {
                    self.toggle_editing();
                }
                KeyCode::Char(value) => match self.currently_editing {
                    CurrentlyEditing::Key => {
                        if let Some(input) = &mut self.key_input {
                            input.push(value);
                        } else {
                            self.key_input = Some(value.to_string());
                        }
                    }
                    CurrentlyEditing::Value => {
                        if let Some(input) = &mut self.value_input {
                            input.push(value);
                        } else {
                            self.value_input = Some(value.to_string());
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
        }
    }

    pub fn should_exit(&self) -> bool {
        self.exit
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(area);

        // Draw title
        Paragraph::new(Text::styled(
            "Device Monitor",
            Style::default().fg(Color::White),
        ))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::empty()))
        .render(chunks[0], buf);

        let mut list_items = Vec::<ListItem>::new();
        for key in self.pairs.keys() {
            list_items.push(ListItem::new(Line::from(Span::styled(
                format!("{:<25}: {}", key, self.pairs.get(key).unwrap()),
                Style::default().fg(Color::Yellow),
            ))));
        }
        List::new(list_items)
            .block(Block::default().borders(Borders::NONE))
            .render(chunks[1], buf);

        let foot_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[2]);

        let mut connect_info = Span::styled(" Disconnected ", Style::default().fg(Color::Red));

        let is_device_connected = self.mqtt_ctrl.is_device_connected();
        jdebug!(
            func = "render()",
            line = line!(),
            device_connected = format!("{:?}", is_device_connected)
        );

        if is_device_connected {
            connect_info = Span::styled(" Connected ", Style::default().fg(Color::Green));
        }

        let current_navigation_text = vec![
            connect_info,
            Span::styled(" | ", Style::default().fg(Color::White)),
            match self.currently_editing {
                CurrentlyEditing::Key => {
                    Span::styled("Editing Json Key", Style::default().fg(Color::Green))
                }
                CurrentlyEditing::Value => {
                    Span::styled("Editing Json Value", Style::default().fg(Color::LightGreen))
                }
                CurrentlyEditing::None => {
                    Span::styled("Not Editing", Style::default().fg(Color::DarkGray))
                }
            },
        ];

        Paragraph::new(Line::from(current_navigation_text))
            .block(Block::default().borders(Borders::NONE))
            .render(foot_chunks[0], buf);

        let current_keys_hint = match self.current_screen {
            CurrentScreen::Main => Span::styled(
                "(q) to quit / (e) to make new pair",
                Style::default().fg(Color::Red),
            ),

            CurrentScreen::Editing => Span::styled(
                "(ESC) to cancel / (Tab) to switch box/ Enter to complete",
                Style::default().fg(Color::Red),
            ),
            CurrentScreen::Exiting => Span::styled(
                "(y) print json / (n) not print json",
                Style::default().fg(Color::Red),
            ),
        };

        Paragraph::new(Line::from(current_keys_hint))
            .block(Block::default().borders(Borders::NONE))
            .render(foot_chunks[1], buf);

        if self.currently_editing != CurrentlyEditing::None {
            let pop_area = centered_rect(60, 25, area);

            Block::default()
                .title("Enter a new key-value pair")
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::DarkGray))
                .render(pop_area, buf);

            let popup_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .margin(2)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(pop_area);

            let mut key_block = Block::default().title("Key").borders(Borders::ALL);
            let mut value_block = Block::default().title("Value").borders(Borders::ALL);
            let active_style = Style::default().bg(Color::LightYellow).fg(Color::Black);
            match self.currently_editing {
                CurrentlyEditing::Key => key_block = key_block.style(active_style),
                CurrentlyEditing::Value => value_block = value_block.style(active_style),
                _ => {}
            };

            Paragraph::new(
                self.key_input
                    .as_ref()
                    .map(|a| a.to_string())
                    .unwrap_or_default(),
            )
            .block(key_block)
            .render(popup_chunks[0], buf);

            Paragraph::new(
                self.value_input
                    .as_ref()
                    .map(|a| a.to_string())
                    .unwrap_or_default(),
            )
            .block(value_block)
            .render(popup_chunks[1], buf);
        }
    }
}

pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<(), DMError> {
    jdebug!(func = "run_app", line = line!(), note = "Main loop");
    loop {
        if app.should_exit() {
            break;
        }

        app.update()?;

        terminal
            .draw(|frame| app.draw(frame))
            .map_err(|e| Report::new(DMError::IOError).attach_printable(e))?;

        app.handle_events()?;
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

    JloggerBuilder::new()
        .max_level(level)
        .log_file(Some(("/tmp/device-monitor", false)))
        .log_console(false)
        .log_time(LogTimeFormat::TimeLocal)
        .build();

    jdebug!(func = "main", line = line!());
    // Initial terminal
    enable_raw_mode().map_err(|e| Report::new(DMError::IOError).attach_printable(e))?;

    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)
        .map_err(|e| Report::new(DMError::IOError).attach_printable(e))?;

    let backend = CrosstermBackend::new(stderr);
    let mut terminal =
        Terminal::new(backend).map_err(|e| Report::new(DMError::IOError).attach_printable(e))?;
    let mut app = App::new(&cli)?;
    let app_result = run_app(&mut terminal, &mut app);

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
        .map_err(|e| Report::new(DMError::IOError).attach_printable(e))?;

    match app_result {
        Ok(_) => app.print_json()?,
        Err(e) => jerror!("{:?}", e),
    }

    Ok(())
}
