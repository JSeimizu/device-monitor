pub mod evp;

#[allow(unused)]
use {
    super::app::{App, ConfigKey, DirectCommand, MainWindowFocus},
    super::error::DMError,
    chrono::{DateTime, Local},
    core::result::Result as CoreResult,
    error_stack::{Report, Result},
    evp::EvpMsg,
    evp::configure::*,
    evp::device_info::{
        DeviceCapabilities, DeviceInfo, DeviceReserved, DeviceStates, NetworkSettings,
        SystemSettings, WirelessSettings,
    },
    evp::evp_state::{AgentDeviceConfig, AgentSystemInfo, UUID},
    evp::rpc::RpcResInfo,
    jlogger_tracing::{JloggerBuilder, LevelFilter, LogTimeFormat, jdebug, jerror, jinfo},
    json::{JsonValue, object::Object},
    rand::Rng,
    regex::Regex,
    rumqttc::{Client, Connection, Event, MqttOptions, QoS},
    std::{
        collections::HashMap,
        sync::Arc,
        sync::atomic::AtomicBool,
        sync::mpsc,
        time::{self, Duration, Instant},
    },
};

pub struct MqttCtrl {
    client: Client,
    thread: Option<std::thread::JoinHandle<()>>,
    rx: mpsc::Receiver<CoreResult<CoreResult<Event, rumqttc::ConnectionError>, rumqttc::RecvError>>,
    should_exit: Arc<AtomicBool>,
    subscribed: bool,
    device_connected: bool,
    last_connected: DateTime<Local>,
    device_info: DeviceInfo,
    device_states: DeviceStates,
    device_capabilities: DeviceCapabilities,
    system_settings: SystemSettings,
    network_settings: NetworkSettings,
    wireless_settings: WirelessSettings,
    device_reserved: DeviceReserved,
    agent_system_info: AgentSystemInfo,
    agent_device_config: AgentDeviceConfig,
    direct_command: Option<DirectCommand>,
    direct_command_start: Option<Instant>,
    direct_command_end: Option<Instant>,
    direct_command_result: Option<Result<String, DMError>>,
    current_rpc_id: u32,
}

fn mqtt_recv_thread(
    mut conn: Connection,
    sender: mpsc::Sender<
        CoreResult<CoreResult<Event, rumqttc::ConnectionError>, rumqttc::RecvError>,
    >,
    should_exit: Arc<AtomicBool>,
) -> Result<(), DMError> {
    let mut network_options = rumqttc::NetworkOptions::default();
    network_options.set_connection_timeout(5);
    conn.eventloop.set_network_options(network_options);

    while !should_exit.load(std::sync::atomic::Ordering::SeqCst) {
        if let Err(e) = sender.send(conn.recv()) {
            jerror!(
                func = "mqtt_recv_thread",
                line = line!(),
                error = format!("{e}")
            );
        }
    }
    Ok(())
}

impl MqttCtrl {
    pub fn new(url: &str, port: u16) -> Result<Self, DMError> {
        let mut rng = rand::rng();
        let id = format!(
            "device-monitor-{}-{}-{}-{}",
            rng.random_range(..1000_u32),
            rng.random_range(..1000_u32),
            rng.random_range(..1000_u32),
            rng.random_range(..1000_u32),
        );

        let mut mqtt_options = MqttOptions::new(id, url, port);
        mqtt_options.set_keep_alive(Duration::from_secs(60));

        jdebug!(
            func = "MqttCtrl::new()",
            line = line!(),
            url = url,
            port = port
        );

        let (client, conn) = Client::new(mqtt_options, 10);
        let (tx, rx) = mpsc::channel();
        let should_exit = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let should_exit_clone = should_exit.clone();
        let thread = std::thread::spawn(move || {
            mqtt_recv_thread(conn, tx, should_exit_clone).unwrap_or_else(|e| {
                jerror!(
                    func = "mqtt_recv_thread",
                    line = line!(),
                    error = format!("{e}")
                );
            });
        });

        let mut subscribed = false;
        if let Ok(_) = client
            .subscribe("#", QoS::AtLeastOnce)
            .map_err(|e| Report::new(DMError::IOError).attach_printable(e))
        {
            subscribed = true;
            jdebug!(
                func = "MqttCtrl::new()",
                line = line!(),
                note = "All topic subscribed"
            );
        }

        Ok(Self {
            client,
            thread: Some(thread),
            rx,
            should_exit,
            subscribed,
            device_connected: false,
            last_connected: Local::now(),
            device_info: DeviceInfo::default(),
            device_states: DeviceStates::default(),
            device_capabilities: DeviceCapabilities::default(),
            device_reserved: DeviceReserved::default(),
            system_settings: SystemSettings::default(),
            network_settings: NetworkSettings::default(),
            wireless_settings: WirelessSettings::default(),
            agent_system_info: AgentSystemInfo::default(),
            agent_device_config: AgentDeviceConfig::default(),
            direct_command: None,
            direct_command_start: None,
            direct_command_end: None,
            direct_command_result: None,
            current_rpc_id: 1000,
        })
    }

    pub fn is_device_connected(&self) -> bool {
        self.device_connected
    }

    pub fn update_timestamp(&mut self) {
        if !self.device_connected {
            self.device_connected = true;
        }
        self.last_connected = Local::now();
    }

    pub fn parse_configure(
        &self,
        config_keys: Option<&Vec<String>>,
        focus: MainWindowFocus,
    ) -> Result<String, DMError> {
        if let Some(config_keys) = config_keys {
            // Agent State
            if focus == MainWindowFocus::AgentState {
                let json = parse_evp_device_config(self.agent_device_config(), config_keys)?;
                if !json.is_empty() {
                    return Ok(json);
                }
            }

            // SystemSettings
            if focus == MainWindowFocus::SystemSettings {
                let json = parse_system_setting(config_keys)?;
                if !json.is_empty() {
                    return Ok(json);
                }
            }

            // NetworkSettings
            if focus == MainWindowFocus::NetworkSettings {
                let json = parse_network_settings(config_keys)?;
                if !json.is_empty() {
                    return Ok(json);
                }
            }

            // WirelessSetting
            if focus == MainWindowFocus::WirelessSettings {
                let json = parse_wireless_settings(config_keys)?;
                if !json.is_empty() {
                    return Ok(json);
                }
            }

            Ok(String::new())
        } else {
            parse_user_config(focus)
        }
    }

    pub fn send_configure(&mut self, config: &str) -> Result<(), DMError> {
        let topic = "v1/devices/me/attributes";
        jdebug!(
            func = "mqtt_ctrl::send_configure",
            line = line!(),
            topic = topic,
            config = config
        );

        // If set retain to true
        // MQTT broker will cache this setting
        self.client
            .publish(topic, QoS::AtLeastOnce, false, config)
            .map_err(|_| Report::new(DMError::IOError))
    }

    pub fn new_rpc_id(&mut self) -> u32 {
        self.current_rpc_id += 1;
        self.current_rpc_id
    }

    pub fn send_rpc_reboot(&mut self) -> Result<String, DMError> {
        let id = self.new_rpc_id();
        let topic = format!("v1/devices/me/rpc/request/{id}");
        let params = Object::new();
        let payload = json::object! {
            "direct-command-request": {
                "reqid": id.to_string(),
                "method": "reboot",
                "instance": "$system",
                "params": params.dump(),
            }
        };

        let mut root = Object::new();
        root.insert("params", payload);

        jdebug!(
            func = "mqtt_ctrl::send_rpc_reboot",
            line = line!(),
            topic = topic,
            payload = root.dump(),
        );

        self.client
            .publish(topic, QoS::AtLeastOnce, false, root.dump())
            .map_err(|_| {
                Report::new(DMError::IOError).attach_printable("Failed to send reboot command")
            })?;
        Ok(root.dump())
    }

    pub fn direct_command_exec_time(&self) -> Option<u32> {
        if let (Some(start), Some(end)) = (self.direct_command_start, self.direct_command_end) {
            Some(end.duration_since(start).as_millis() as u32)
        } else if let Some(start) = self.direct_command_start {
            Some(start.elapsed().as_millis() as u32)
        } else {
            None
        }
    }

    pub fn on_message(
        &mut self,
        topic: &str,
        payload: &str,
    ) -> Result<HashMap<String, String>, DMError> {
        let mut result = HashMap::new();

        for msg in EvpMsg::parse(topic, payload)? {
            match msg {
                EvpMsg::ConnectMsg((who, req_id)) => {
                    self.client
                        .publish(
                            &format!("v1/devices/{who}/attributes/response/{req_id}"),
                            QoS::AtLeastOnce,
                            false,
                            payload,
                        )
                        .map_err(|_| Report::new(DMError::IOError))?;

                    result.insert(
                        "Connection request".to_owned(),
                        format!("who={who} req_id={req_id}"),
                    );
                    self.update_timestamp();
                }
                EvpMsg::ConnectRespMsg((who, req_id)) => {
                    result.insert(
                        "Connection response".to_owned(),
                        format!("who={who} req_id={req_id}"),
                    );
                }
                EvpMsg::DeviceInfoMsg(device_info) => {
                    self.device_info = device_info;
                    self.update_timestamp();
                }
                EvpMsg::DeviceStatesMsg(device_states) => {
                    self.device_states = device_states;
                    self.update_timestamp();
                }
                EvpMsg::DeviceCapabilities(device_capabilities) => {
                    self.device_capabilities = device_capabilities;
                    self.update_timestamp();
                }
                EvpMsg::DeviceReserved(device_reserved) => {
                    self.device_reserved = device_reserved;
                    self.update_timestamp();
                }
                EvpMsg::SystemSettings(system_settings) => {
                    self.system_settings = system_settings;
                    self.update_timestamp();
                }
                EvpMsg::NetworkSettings(network_settings) => {
                    self.network_settings = network_settings;
                    self.update_timestamp();
                }
                EvpMsg::WirelessSettings(wireless_settings) => {
                    self.wireless_settings = wireless_settings;
                    self.update_timestamp();
                }
                EvpMsg::AgentSystemInfo(system_info) => {
                    self.agent_system_info = system_info;
                    self.update_timestamp();
                }
                EvpMsg::AgentDeviceConfig(config) => {
                    self.agent_device_config = config;
                    self.update_timestamp();
                }
                EvpMsg::ClientMsg(v) => {
                    self.update_timestamp();
                    result.extend(v);
                }
                EvpMsg::ServerMsg(v) => {
                    result.extend(v);
                }
                EvpMsg::RpcRequest(v) => {
                    let (req_id, cmd) = v;
                    jinfo!(
                        event = "DirectCommand request",
                        req_id = req_id,
                        direct_command = cmd.to_string()
                    );
                }
                EvpMsg::RpcResponse(v) => {
                    let (req_id, response) = v;
                    jdebug!(
                        func = "mqtt_ctrl::on_message()",
                        line = line!(),
                        req_id = req_id,
                        current_rpc_id = self.current_rpc_id,
                        response = response.to_string()
                    );
                    if req_id == self.current_rpc_id {
                        self.direct_command_result = Some(Ok(response.to_string()));
                        self.direct_command_end = Some(Instant::now());

                        if let (Some(start), Some(end)) =
                            (self.direct_command_start, self.direct_command_end)
                        {
                            jinfo!(
                                event = "TIME_MEASURE",
                                direct_command =
                                    format!("{}ms", end.duration_since(start).as_millis())
                            );
                        }
                    }

                    self.update_timestamp();
                }
                EvpMsg::NonEvp(v) => {
                    result.extend(v);
                }
            };
        }

        Ok(result)
    }

    pub fn update(&mut self) -> Result<HashMap<String, String>, DMError> {
        let mut result = HashMap::new();

        if !self.subscribed {
            self.client
                .subscribe("#", QoS::AtLeastOnce)
                .map_err(|e| Report::new(DMError::IOError).attach_printable(e))?;
        }

        // If DirectCommand is set, we are in a DirectCommand screen.
        if let Some(cmd) = self.direct_command.as_ref() {
            match cmd {
                DirectCommand::Reboot => {
                    if let Some(start) = self.direct_command_start {
                        jdebug!(
                            func = "App::update()",
                            event = "Reboot",
                            time = format!("{}ms", start.elapsed().as_millis())
                        );
                    } else {
                        jdebug!(func = "App::handle_key_event()", event = "Start Reboot",);
                        self.direct_command_start = Some(Instant::now());
                        self.direct_command_result = Some(self.send_rpc_reboot());
                    }
                }
                DirectCommand::GetDirectImage => {
                    if let Some(start) = self.direct_command_start {
                        jdebug!(
                            func = "App::update()",
                            event = "GetDirectImage",
                            time = format!("{}ms", start.elapsed().as_millis())
                        );
                    } else {
                        self.direct_command_start = Some(Instant::now());
                    }
                }
                DirectCommand::FactoryReset => {
                    if let Some(start) = self.direct_command_start {
                        jdebug!(
                            func = "App::update()",
                            event = "FactoryReset",
                            time = format!("{}ms", start.elapsed().as_millis())
                        );
                    } else {
                        self.direct_command_start = Some(Instant::now());
                    }
                }
                _ => {}
            }
        }

        match self.rx.try_recv() {
            Ok(v) => match v {
                Ok(event) => match event {
                    Ok(rumqttc::Event::Incoming(i_event)) => match i_event {
                        rumqttc::Packet::Publish(data) => {
                            jdebug!(func = "MqttCtrl::read()", line = line!(), note = "publish");
                            let topic = data.topic;
                            let payload = String::from_utf8(data.payload.to_vec())
                                .map_err(|_e| Report::new(DMError::InvalidData))?;

                            result.extend(self.on_message(&topic, &payload)?);
                        }
                        _ => {
                            jdebug!(func = "MqttCtrl::read()", line = line!(), note = "others");
                        }
                    },
                    Ok(rumqttc::Event::Outgoing(_o_event)) => {}
                    Err(e) => {
                        jerror!(
                            func = "MqttCtrl::read()",
                            line = line!(),
                            error = format!("{e}")
                        );
                        return Err(Report::new(DMError::IOError)
                            .attach_printable("Failed connecting to MQTT broker, reboot needed"));
                    }
                },
                Err(_) => {
                    jerror!(
                        func = "MqttCtrl::update()",
                        line = line!(),
                        error = "RecvError",
                    );

                    return Err(Report::new(DMError::IOError)
                        .attach_printable("Failed receiving from MQTT broker"));
                }
            },
            Err(_) => {}
        }

        // EVP agent will send state at least report_status_interval_max seconds
        // Here a threshold value in seconds of
        //    report_status_interval_max + 60
        // is used to judge whether device is disconnected.
        // That is, if no messages have been sent from device, the device is considered to be
        // disconnected.
        let threshold = (self.agent_device_config.report_status_interval_max + 60) as i64;
        if (Local::now() - self.last_connected).num_seconds() > threshold {
            self.device_connected = false;
        }

        Ok(result)
    }

    pub fn device_info(&self) -> &DeviceInfo {
        &self.device_info
    }

    pub fn agent_system_info(&self) -> &AgentSystemInfo {
        &self.agent_system_info
    }

    pub fn agent_device_config(&self) -> &AgentDeviceConfig {
        &self.agent_device_config
    }

    pub fn last_connected_time(&self) -> DateTime<Local> {
        self.last_connected
    }

    pub fn last_connected(&self) -> String {
        self.last_connected.format("%Y-%m-%d %H:%M:%S").to_string()
    }

    pub fn device_states(&self) -> &DeviceStates {
        &self.device_states
    }

    pub fn device_capabilities(&self) -> &DeviceCapabilities {
        &self.device_capabilities
    }

    pub fn device_reserved(&self) -> &DeviceReserved {
        &self.device_reserved
    }

    pub fn system_settings(&self) -> &SystemSettings {
        &self.system_settings
    }

    pub fn network_settings(&self) -> &NetworkSettings {
        &self.network_settings
    }

    pub fn wireless_settings(&self) -> &WirelessSettings {
        &self.wireless_settings
    }

    pub fn exit(&mut self) {
        self.should_exit
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.thread.take().map(|t| t.join());
    }

    pub fn set_direct_command(&mut self, direct_command: Option<DirectCommand>) {
        self.direct_command = direct_command;
    }

    pub fn get_direct_command(&self) -> Option<DirectCommand> {
        self.direct_command.clone()
    }

    pub fn direct_command_result(&self) -> Option<&Result<String, DMError>> {
        self.direct_command_result.as_ref()
    }

    pub fn direct_command_clear(&mut self) {
        self.direct_command = None;
        self.direct_command_result = None;
        self.direct_command_start = None;
        self.direct_command_end = None;
    }
}
