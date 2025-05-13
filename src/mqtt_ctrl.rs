pub mod evp;

#[allow(unused)]
use {
    super::app::{ConfigKey, MainWindowFocus},
    super::error::DMError,
    chrono::{DateTime, Local},
    error_stack::{Report, Result},
    evp::EvpMsg,
    evp::configure::*,
    evp::device_info::{
        DeviceCapabilities, DeviceInfo, DeviceReserved, DeviceStates, NetworkSettings,
        SystemSettings, WirelessSettings,
    },
    evp::evp_state::{AgentDeviceConfig, AgentSystemInfo},
    jlogger_tracing::{JloggerBuilder, LevelFilter, LogTimeFormat, jdebug, jerror, jinfo},
    rand::Rng,
    regex::Regex,
    rumqttc::{Client, Connection, MqttOptions, QoS},
    std::{
        collections::HashMap,
        time::{self, Duration, Instant},
    },
};

pub struct MqttCtrl {
    client: Client,
    conn: Connection,
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
        config_keys: &Vec<String>,
        focus: MainWindowFocus,
    ) -> Result<String, DMError> {
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
    }

    pub fn send_configure(&mut self, config: &str) -> Result<(), DMError> {
        let topic = "v1/devices/me/attributes";
        jdebug!(
            func = "mqtt_ctrl::send_configure",
            line = line!(),
            topic = topic,
            config = config
        );

        // set retain to true
        // MQTT broker will cache this setting
        self.client
            .publish(topic, QoS::AtLeastOnce, true, config)
            .map_err(|_| Report::new(DMError::IOError))
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
                EvpMsg::RpcServer(v) => {
                    result.extend(v);
                }
                EvpMsg::RpcClient(v) => {
                    self.update_timestamp();
                    result.extend(v);
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
        //jdebug!(func = "MqttCtrl::read()", line = line!());

        match self.conn.recv_timeout(Duration::from_millis(100)) {
            Ok(v) => match v {
                Ok(event) => match event {
                    rumqttc::Event::Incoming(i_event) => match i_event {
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
                    rumqttc::Event::Outgoing(_o_event) => {}
                },
                Err(e) => {
                    jdebug!(
                        func = "MqttCtrl::read()",
                        line = line!(),
                        error = format!("{e}")
                    );
                }
            },
            Err(_e) => {}
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
}
