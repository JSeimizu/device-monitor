pub mod evp;

#[allow(unused)]
use {
    super::error::DMError,
    chrono::{DateTime, Local},
    error_stack::{Report, Result},
    evp::EvpMsg,
    evp::device_info::{DeviceCapabilities, DeviceInfo, DeviceReserved, DeviceStates},
    evp::evp_state::{AgentDeviceConfig, AgentSystemInfo},
    jlogger_tracing::{JloggerBuilder, LevelFilter, LogTimeFormat, jdebug, jerror, jinfo},
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
    device_reserved: DeviceReserved,
    agent_system_info: AgentSystemInfo,
    agent_device_config: AgentDeviceConfig,
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
            last_connected: Local::now(),
            device_info: DeviceInfo::default(),
            device_states: DeviceStates::default(),
            device_capabilities: DeviceCapabilities::default(),
            device_reserved: DeviceReserved::default(),
            agent_system_info: AgentSystemInfo::default(),
            agent_device_config: AgentDeviceConfig::default(),
        })
    }

    pub fn is_device_connected(&self) -> bool {
        self.device_connected
    }

    pub fn update_timestamp(&mut self) {
        self.device_connected = true;
        self.last_connected = Local::now();
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
                EvpMsg::AgentSystemInfo(system_info) => {
                    self.agent_system_info = system_info;
                    self.update_timestamp();
                }
                EvpMsg::AgentDeviceConfig(config) => {
                    self.agent_device_config = config;
                    self.update_timestamp();
                }
                EvpMsg::ClientMsg(v) => {
                    self.last_connected = Local::now();
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

        // If there is no messages from device for 5 minutes
        // device is considered to be disconnected.
        let now = Local::now();
        let delta = now - self.last_connected;
        if delta.num_seconds() > 5 * 60 {
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
}
