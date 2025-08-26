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

pub mod evp;

use crate::ai_model::AiModel;
use azure_storage::prelude::BlobSasPermissions;
use std::sync::{Mutex, OnceLock};

static GLOBAL_MQTT_CTRL: OnceLock<Mutex<MqttCtrl>> = OnceLock::new();

pub fn init_global_mqtt_ctrl(broker: &str) -> Result<(), DMError> {
    let (broker_url, broker_port_str) = broker.split_once(':').unwrap_or((broker, "1883"));
    let broker_port = broker_port_str.parse().map_err(|_| {
        Report::new(DMError::InvalidData)
            .attach_printable(format!("Invalid broker port: {}", broker_port_str))
    })?;

    let mqtt_ctrl = MqttCtrl::new(broker_url, broker_port)?;
    GLOBAL_MQTT_CTRL.set(Mutex::new(mqtt_ctrl)).map_err(|_| {
        Report::new(DMError::RuntimeError)
            .attach_printable("Failed to initialize global MqttCtrl - already initialized")
    })
}

pub fn with_mqtt_ctrl<F, R>(f: F) -> R
where
    F: FnOnce(&MqttCtrl) -> R,
{
    let mqtt_ctrl = GLOBAL_MQTT_CTRL
        .get()
        .expect("Global MqttCtrl not initialized")
        .lock()
        .expect("Failed to lock global MqttCtrl mutex");
    f(&mqtt_ctrl)
}

pub fn with_mqtt_ctrl_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut MqttCtrl) -> R,
{
    let mut mqtt_ctrl = GLOBAL_MQTT_CTRL
        .get()
        .expect("Global MqttCtrl not initialized")
        .lock()
        .expect("Failed to lock global MqttCtrl mutex");
    f(&mut mqtt_ctrl)
}

// Temporary function to get a reference to the global MqttCtrl for UI compatibility
// This is not ideal but allows us to migrate gradually
#[allow(dead_code)]
pub fn get_global_mqtt_ctrl_ref() -> &'static std::sync::Mutex<MqttCtrl> {
    GLOBAL_MQTT_CTRL
        .get()
        .expect("Global MqttCtrl not initialized")
}

#[allow(unused)]
use {
    super::app::{App, ConfigKey, DirectCommand, MainWindowFocus},
    super::error::DMError,
    super::ota::FirmwareProperty,
    crate::{app::with_global_app, azurite::with_azurite_storage},
    base64::{
        Engine as _, alphabet,
        engine::{self, general_purpose},
    },
    chrono::{DateTime, Local},
    core::result::Result as CoreResult,
    error_stack::{Report, Result},
    evp::EvpMsg,
    evp::configure::*,
    evp::device_info::{
        DeviceCapabilities, DeviceInfo, DeviceReserved, DeviceStates, NetworkSettings,
        SystemSettings, WirelessSettings,
    },
    evp::edge_app_passthrough::EdgeAppPassthrough,
    evp::elog::Elog,
    evp::evp_state::{AgentDeviceConfig, AgentSystemInfo, DeploymentStatus, UUID},
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
    #[allow(dead_code)]
    thread: Option<std::thread::JoinHandle<()>>,
    rx: mpsc::Receiver<CoreResult<CoreResult<Event, rumqttc::ConnectionError>, rumqttc::RecvError>>,
    #[allow(dead_code)]
    should_exit: Arc<AtomicBool>,
    subscribed: bool,
    device_connected: bool,
    last_connected: DateTime<Local>,
    device_info: Option<DeviceInfo>,
    device_states: Option<DeviceStates>,
    device_capabilities: Option<DeviceCapabilities>,
    system_settings: Option<SystemSettings>,
    network_settings: Option<Box<NetworkSettings>>,
    wireless_settings: Option<WirelessSettings>,
    device_reserved: Option<DeviceReserved>,
    agent_system_info: Option<Box<AgentSystemInfo>>,
    deployment_status: Option<DeploymentStatus>,
    agent_device_config: Option<AgentDeviceConfig>,
    edge_app_passthrough: EdgeAppPassthrough,
    direct_command: Option<DirectCommand>,
    direct_command_start: Option<Instant>,
    direct_command_end: Option<Instant>,
    direct_command_request: Option<Result<String, DMError>>,
    direct_command_result: Option<Result<RpcResInfo, DMError>>,
    current_rpc_id: u32,
    elogs: Vec<Elog>,
    firmware: FirmwareProperty,
    ai_model: AiModel,
    pub info: Option<String>,
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
        mqtt_options.set_max_packet_size(262144, 262144);

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
        if client.subscribe("#", QoS::AtLeastOnce).is_ok() {
            subscribed = true;
            jdebug!(
                func = "MqttCtrl::new()",
                line = line!(),
                note = "All topic subscribed"
            );
        }

        let current_rpc_id = rng.random_range(10000..99999);

        Ok(Self {
            client,
            thread: Some(thread),
            rx,
            should_exit,
            subscribed,
            device_connected: false,
            last_connected: Local::now(),
            device_info: None,
            device_states: None,
            device_capabilities: None,
            device_reserved: None,
            system_settings: None,
            network_settings: None,
            wireless_settings: None,
            agent_system_info: None,
            elogs: Vec::new(),
            deployment_status: None,
            agent_device_config: None,
            edge_app_passthrough: EdgeAppPassthrough::new(),
            direct_command: None,
            direct_command_start: None,
            direct_command_end: None,
            direct_command_request: None,
            direct_command_result: None,
            current_rpc_id,
            info: None,
            firmware: FirmwareProperty::new(),
            ai_model: AiModel::new(),
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

    pub fn parse_edge_app_passthrough_config(
        &self,
        config_keys: &[String],
    ) -> Result<String, DMError> {
        use crate::app::ConfigKey;
        use uuid::Uuid;

        // Generate UUID for req_info
        let req_id = Uuid::new_v4().to_string();

        // Helper function to parse string to integer
        let parse_int = |s: &str| -> Result<i32, DMError> {
            s.parse().map_err(|_| Report::new(DMError::InvalidData))
        };

        // Helper function to parse string to float
        let parse_float = |s: &str| -> Result<f64, DMError> {
            s.parse().map_err(|_| Report::new(DMError::InvalidData))
        };

        // Helper function to parse string to boolean
        let parse_bool = |s: &str| -> Result<bool, DMError> {
            match s.to_lowercase().as_str() {
                "true" | "1" => Ok(true),
                "false" | "0" => Ok(false),
                _ => Err(Report::new(DMError::InvalidData)),
            }
        };

        // Helper function to get config value by key
        let get_config = |key: ConfigKey| -> &str {
            config_keys
                .get(usize::from(key))
                .map(|s| s.as_str())
                .unwrap_or("")
        };

        let mut edge_app = serde_json::json!({
            "edge_app": {
                "req_info": {
                    "req_id": req_id
                },
                "common_settings": {
                    "process_state": null,
                    "log_level": null,
                    "inference_settings": {
                        "number_of_iterations": null
                    },
                    "pq_settings": {
                        "camera_image_size": {
                            "width": null,
                            "height": null,
                            "scaling_policy": null
                        },
                        "frame_rate": {
                            "num": null,
                            "denom": null
                        },
                        "digital_zoom": null,
                        "camera_image_flip": {
                            "flip_horizontal": null,
                            "flip_vertical": null
                        },
                        "exposure_mode": null,
                        "auto_exposure": {
                            "max_exposure_time": null,
                            "min_exposure_time": null,
                            "max_gain": null,
                            "convergence_speed": null
                        },
                        "auto_exposure_metering": {
                            "metering_mode": null,
                            "top": null,
                            "left": null,
                            "bottom": null,
                            "right": null
                        },
                        "ev_compensation": null,
                        "ae_anti_flicker_mode": null,
                        "manual_exposure": {
                            "exposure_time": null,
                            "gain": null
                        },
                        "white_balance_mode": null,
                        "auto_white_balance": {
                            "convergence_speed": null
                        },
                        "manual_white_balance_preset": {
                            "color_temperature": null
                        },
                        "image_cropping": {
                            "left": null,
                            "top": null,
                            "width": null,
                            "height": null
                        },
                        "image_rotation": null
                    },
                    "port_settings": {
                        "metadata": {
                            "method": null,
                            "storage_name": null,
                            "endpoint": null,
                            "path": null,
                            "enabled": null
                        },
                        "input_tensor": {
                            "method": null,
                            "storage_name": null,
                            "endpoint": null,
                            "path": null,
                            "enabled": null
                        }
                    },
                    "codec_settings": {
                        "format": null
                    },
                    "number_of_inference_per_message": null
                },
                "custom_settings": {
                    "ai_models": {}
                }
            }
        });

        // Parse and validate each field
        let process_state = get_config(ConfigKey::EdgeAppPassthroughProcessState);
        if !process_state.is_empty() {
            edge_app["edge_app"]["common_settings"]["process_state"] =
                serde_json::Value::Number(parse_int(process_state)?.into());
        }

        let log_level = get_config(ConfigKey::EdgeAppPassthroughLogLevel);
        if !log_level.is_empty() {
            edge_app["edge_app"]["common_settings"]["log_level"] =
                serde_json::Value::Number(parse_int(log_level)?.into());
        }

        let iterations = get_config(ConfigKey::EdgeAppPassthroughNumberOfIterations);
        if !iterations.is_empty() {
            edge_app["edge_app"]["common_settings"]["inference_settings"]["number_of_iterations"] =
                serde_json::Value::Number(parse_int(iterations)?.into());
        }

        // Camera image size
        let width = get_config(ConfigKey::EdgeAppPassthroughCameraImageSizeWidth);
        if !width.is_empty() {
            edge_app["edge_app"]["common_settings"]["pq_settings"]["camera_image_size"]["width"] =
                serde_json::Value::Number(parse_int(width)?.into());
        }

        let height = get_config(ConfigKey::EdgeAppPassthroughCameraImageSizeHeight);
        if !height.is_empty() {
            edge_app["edge_app"]["common_settings"]["pq_settings"]["camera_image_size"]["height"] =
                serde_json::Value::Number(parse_int(height)?.into());
        }

        let scaling_policy = get_config(ConfigKey::EdgeAppPassthroughCameraImageSizeScalingPolicy);
        if !scaling_policy.is_empty() {
            edge_app["edge_app"]["common_settings"]["pq_settings"]["camera_image_size"]["scaling_policy"] =
                serde_json::Value::Number(parse_int(scaling_policy)?.into());
        }

        // Frame rate
        let frame_num = get_config(ConfigKey::EdgeAppPassthroughFrameRateNum);
        if !frame_num.is_empty() {
            edge_app["edge_app"]["common_settings"]["pq_settings"]["frame_rate"]["num"] =
                serde_json::Value::Number(parse_int(frame_num)?.into());
        }

        let frame_denom = get_config(ConfigKey::EdgeAppPassthroughFrameRateDenom);
        if !frame_denom.is_empty() {
            edge_app["edge_app"]["common_settings"]["pq_settings"]["frame_rate"]["denom"] =
                serde_json::Value::Number(parse_int(frame_denom)?.into());
        }

        let digital_zoom = get_config(ConfigKey::EdgeAppPassthroughDigitalZoom);
        if !digital_zoom.is_empty() {
            edge_app["edge_app"]["common_settings"]["pq_settings"]["digital_zoom"] =
                serde_json::Value::Number(
                    serde_json::Number::from_f64(parse_float(digital_zoom)?)
                        .ok_or_else(|| Report::new(DMError::InvalidData))?,
                );
        }

        // Camera image flip
        let flip_h = get_config(ConfigKey::EdgeAppPassthroughCameraImageFlipHorizontal);
        if !flip_h.is_empty() {
            edge_app["edge_app"]["common_settings"]["pq_settings"]["camera_image_flip"]["flip_horizontal"] =
                serde_json::Value::Number(parse_int(flip_h)?.into());
        }

        let flip_v = get_config(ConfigKey::EdgeAppPassthroughCameraImageFlipVertical);
        if !flip_v.is_empty() {
            edge_app["edge_app"]["common_settings"]["pq_settings"]["camera_image_flip"]["flip_vertical"] =
                serde_json::Value::Number(parse_int(flip_v)?.into());
        }

        let exposure_mode = get_config(ConfigKey::EdgeAppPassthroughExposureMode);
        if !exposure_mode.is_empty() {
            edge_app["edge_app"]["common_settings"]["pq_settings"]["exposure_mode"] =
                serde_json::Value::Number(parse_int(exposure_mode)?.into());
        }

        // Continue with other fields as needed...
        // Port settings metadata
        let meta_method = get_config(ConfigKey::EdgeAppPassthroughMetadataMethod);
        if !meta_method.is_empty() {
            edge_app["edge_app"]["common_settings"]["port_settings"]["metadata"]["method"] =
                serde_json::Value::Number(parse_int(meta_method)?.into());
        }

        let meta_storage = get_config(ConfigKey::EdgeAppPassthroughMetadataStorageName);
        if !meta_storage.is_empty() {
            edge_app["edge_app"]["common_settings"]["port_settings"]["metadata"]["storage_name"] =
                serde_json::Value::String(meta_storage.to_string());
        }

        let meta_endpoint = get_config(ConfigKey::EdgeAppPassthroughMetadataEndpoint);
        if !meta_endpoint.is_empty() {
            edge_app["edge_app"]["common_settings"]["port_settings"]["metadata"]["endpoint"] =
                serde_json::Value::String(meta_endpoint.to_string());
        }

        let meta_path = get_config(ConfigKey::EdgeAppPassthroughMetadataPath);
        if !meta_path.is_empty() {
            edge_app["edge_app"]["common_settings"]["port_settings"]["metadata"]["path"] =
                serde_json::Value::String(meta_path.to_string());
        }

        let meta_enabled = get_config(ConfigKey::EdgeAppPassthroughMetadataEnabled);
        if !meta_enabled.is_empty() {
            edge_app["edge_app"]["common_settings"]["port_settings"]["metadata"]["enabled"] =
                serde_json::Value::Bool(parse_bool(meta_enabled)?);
        }

        // Input tensor settings
        let it_method = get_config(ConfigKey::EdgeAppPassthroughInputTensorMethod);
        if !it_method.is_empty() {
            edge_app["edge_app"]["common_settings"]["port_settings"]["input_tensor"]["method"] =
                serde_json::Value::Number(parse_int(it_method)?.into());
        }

        let it_storage = get_config(ConfigKey::EdgeAppPassthroughInputTensorStorageName);
        if !it_storage.is_empty() {
            edge_app["edge_app"]["common_settings"]["port_settings"]["input_tensor"]["storage_name"] =
                serde_json::Value::String(it_storage.to_string());
        }

        let it_endpoint = get_config(ConfigKey::EdgeAppPassthroughInputTensorEndpoint);
        if !it_endpoint.is_empty() {
            edge_app["edge_app"]["common_settings"]["port_settings"]["input_tensor"]["endpoint"] =
                serde_json::Value::String(it_endpoint.to_string());
        }

        let it_path = get_config(ConfigKey::EdgeAppPassthroughInputTensorPath);
        if !it_path.is_empty() {
            edge_app["edge_app"]["common_settings"]["port_settings"]["input_tensor"]["path"] =
                serde_json::Value::String(it_path.to_string());
        }

        let it_enabled = get_config(ConfigKey::EdgeAppPassthroughInputTensorEnabled);
        if !it_enabled.is_empty() {
            edge_app["edge_app"]["common_settings"]["port_settings"]["input_tensor"]["enabled"] =
                serde_json::Value::Bool(parse_bool(it_enabled)?);
        }

        // Codec settings
        let codec_format = get_config(ConfigKey::EdgeAppPassthroughCodecFormat);
        if !codec_format.is_empty() {
            edge_app["edge_app"]["common_settings"]["codec_settings"]["format"] =
                serde_json::Value::Number(parse_int(codec_format)?.into());
        }

        let infer_per_msg = get_config(ConfigKey::EdgeAppPassthroughNumberOfInferencePerMessage);
        if !infer_per_msg.is_empty() {
            edge_app["edge_app"]["common_settings"]["number_of_inference_per_message"] =
                serde_json::Value::Number(parse_int(infer_per_msg)?.into());
        }

        // Custom settings - AI models
        let ai_model_bundle_id = get_config(ConfigKey::EdgeAppPassthroughAiModelBundleId);
        if !ai_model_bundle_id.is_empty() {
            edge_app["edge_app"]["custom_settings"]["ai_models"]["passthrough"] = serde_json::json!({
                "ai_model_bundle_id": ai_model_bundle_id
            });
        }

        serde_json::to_string_pretty(&edge_app).map_err(|_| Report::new(DMError::InvalidData))
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

    pub fn send_rpc_direct_get_image(&mut self, config_keys: &[String]) -> Result<String, DMError> {
        let id = self.new_rpc_id();
        let topic = format!("v1/devices/me/rpc/request/{id}");
        let params = json::object! {
            "sensor_name": config_keys
                .get(ConfigKey::DirectGetImageSensorName as usize)
                .map_or("", |s| s.as_str()),
            "network_id": config_keys
                .get(ConfigKey::DirectGetImageNetworkId as usize)
                .map_or("", |s| s.as_str()),
        };

        let payload = json::object! {
                "direct-command-request": {
                    "reqid": id.to_string(),
                    "method": "direct_get_image",
                    "instance": "$system",
                    "params": params.dump(),
                }
        };

        let mut root = Object::new();
        root.insert("params", payload);
        let result = root.dump();

        self.direct_command_start = Some(Instant::now());
        self.client
            .publish(topic, QoS::AtLeastOnce, false, result.clone())
            .map_err(|_| {
                Report::new(DMError::IOError).attach_printable("Failed to send reboot command")
            })?;

        self.direct_command_request = Some(Ok(result.clone()));
        Ok(result)
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

        self.direct_command_start = Some(Instant::now());
        self.client
            .publish(topic, QoS::AtLeastOnce, false, root.dump())
            .map_err(|_| {
                Report::new(DMError::IOError).attach_printable("Failed to send reboot command")
            })?;
        Ok(root.dump())
    }

    pub fn send_rpc_factory_reset(&mut self) -> Result<String, DMError> {
        let id = self.new_rpc_id();
        let topic = format!("v1/devices/me/rpc/request/{id}");
        let params = Object::new();
        let payload = json::object! {
            "direct-command-request": {
                "reqid": id.to_string(),
                "method": "factory_reset",
                "instance": "$system",
                "params": params.dump(),
            }
        };

        let mut root = Object::new();
        root.insert("params", payload);

        jdebug!(
            func = "mqtt_ctrl::send_rpc_factory_reset",
            line = line!(),
            topic = topic,
            payload = root.dump(),
        );

        self.direct_command_start = Some(Instant::now());
        self.client
            .publish(topic, QoS::AtLeastOnce, false, root.dump())
            .map_err(|_| {
                Report::new(DMError::IOError)
                    .attach_printable("Failed to send factory_reset command")
            })?;
        Ok(root.dump())
    }

    pub fn direct_command_exec_time(&self) -> Option<u32> {
        if let (Some(start), Some(end)) = (self.direct_command_start, self.direct_command_end) {
            Some(end.duration_since(start).as_millis() as u32)
        } else {
            self.direct_command_start
                .map(|start| start.elapsed().as_millis() as u32)
        }
    }

    pub fn on_message(
        &mut self,
        topic: &str,
        payload: &str,
    ) -> Result<HashMap<String, String>, DMError> {
        let mut result = HashMap::new();

        jdebug!(
            func = "mqtt_ctrl::on_message()",
            line = line!(),
            topic = topic,
            payload = payload
        );

        for msg in EvpMsg::parse(topic, payload, self)? {
            match msg {
                EvpMsg::ConnectMsg((who, req_id)) => {
                    self.client
                        .publish(
                            format!("v1/devices/{who}/attributes/response/{req_id}"),
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
                    self.info = Some("Device rebooted".to_owned());
                }
                EvpMsg::ConnectRespMsg((who, req_id)) => {
                    result.insert(
                        "Connection response".to_owned(),
                        format!("who={who} req_id={req_id}"),
                    );
                }
                EvpMsg::DeviceInfoMsg(device_info) => {
                    self.device_info = Some(device_info);
                    self.update_timestamp();
                }
                EvpMsg::DeviceStatesMsg(device_states) => {
                    self.device_states = Some(device_states);
                    self.update_timestamp();
                }
                EvpMsg::DeviceCapabilities(device_capabilities) => {
                    self.device_capabilities = Some(device_capabilities);
                    self.update_timestamp();
                }
                EvpMsg::DeviceReserved(device_reserved) => {
                    self.device_reserved = Some(device_reserved);
                    self.update_timestamp();
                }
                EvpMsg::SystemSettings(system_settings) => {
                    self.system_settings = Some(system_settings);
                    self.update_timestamp();
                }
                EvpMsg::NetworkSettings(network_settings) => {
                    self.network_settings = Some(network_settings);
                    self.update_timestamp();
                }
                EvpMsg::WirelessSettings(wireless_settings) => {
                    self.wireless_settings = Some(wireless_settings);
                    self.update_timestamp();
                }
                EvpMsg::AgentSystemInfo(system_info) => {
                    self.agent_system_info = Some(system_info);
                    self.update_timestamp();
                }
                EvpMsg::PrivateDeployFirmware(firmware) => {
                    self.firmware = firmware;
                    self.update_timestamp();
                }
                EvpMsg::PrivateDeployAiModel(ai_model) => {
                    self.ai_model = ai_model;
                    self.update_timestamp();
                }
                EvpMsg::DeploymentStatus(deployment_status) => {
                    self.deployment_status = Some(deployment_status);
                    self.update_timestamp();
                }
                EvpMsg::EdgeAppPassthrough((_instance_id, edge_app)) => {
                    self.edge_app_passthrough = edge_app;
                    self.update_timestamp();
                }
                EvpMsg::AgentDeviceConfig(config) => {
                    self.agent_device_config = Some(config);
                    self.update_timestamp();
                }
                EvpMsg::Elog(elog) => {
                    jdebug!(
                        func = "mqtt_ctrl::on_message()",
                        line = line!(),
                        log = ? elog
                    );
                    self.elogs.push(elog);
                    if self.elogs.len() > 100 {
                        self.elogs.remove(0);
                    }
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

                    if let DirectCommand::StorageTokenRequest(key, filename) = cmd {
                        match with_azurite_storage(|azurite| -> Result<(), DMError> {
                            let topic = format!("v1/devices/me/rpc/response/{req_id}");
                            let mut payload = json::object! {
                                "storagetoken-response": {
                                    "reqid": req_id.to_string(),
                                    "status": "error",
                                }
                            };

                            // Validate provided key is a UUID
                            let uuid = match UUID::from(&key) {
                                Ok(u) => u,
                                Err(_) => {
                                    jerror!(
                                        func = "mqtt_ctrl::on_message()",
                                        line = line!(),
                                        event = "Invalid UUID in StorageTokenRequest"
                                    );
                                    // Publish error response
                                    self.client
                                        .publish(topic, QoS::AtLeastOnce, false, payload.dump())
                                        .map_err(|_| Report::new(DMError::IOError))?;
                                    return Ok(());
                                }
                            };

                            // Basic filename validation: non-empty, no traversal, reasonable length
                            if filename.is_empty()
                                || filename.contains("..")
                                || filename.contains('\\')
                            {
                                jerror!(
                                    func = "mqtt_ctrl::on_message()",
                                    line = line!(),
                                    event = "Invalid filename in StorageTokenRequest"
                                );
                                self.client
                                    .publish(topic, QoS::AtLeastOnce, false, payload.dump())
                                    .map_err(|_| Report::new(DMError::IOError))?;
                                return Ok(());
                            }

                            if let Some(token) = azurite.token_providers().get(&uuid) {
                                jdebug!(
                                    func = "mqtt_ctrl::on_message()",
                                    line = line!(),
                                    RPC = "StorageTokenRequest response prepared",
                                    key = &key,
                                    filename = &filename
                                );

                                let token_permissions = BlobSasPermissions {
                                    read: true,
                                    write: true,
                                    add: true,
                                    create: true,
                                    ..Default::default()
                                };
                                // Limit SAS TTL for device uploads to 1 hour
                                let one_hour = std::time::Duration::from_secs(3600);

                                if let Ok(sas_url) = azurite.get_sas_url(
                                    &token.container,
                                    &filename,
                                    Some(token_permissions),
                                    Some(one_hour),
                                ) {
                                    payload = json::object! {
                                        "storagetoken-response": {
                                            "reqid": req_id.to_string(),
                                            "status": "ok".to_string(),
                                            "URL": sas_url,
                                            "headers": {
                                                "x-ms-blob-type": "BlockBlob".to_string()
                                            }
                                        }
                                    };
                                } else {
                                    jerror!(
                                        func = "mqtt_ctrl::on_message()",
                                        line = line!(),
                                        event = "Failed to generate SAS URL"
                                    );
                                }
                            } else {
                                jerror!(
                                    func = "mqtt_ctrl::on_message()",
                                    line = line!(),
                                    event = "Token provider not found",
                                    key = &key
                                );
                            }

                            self.client
                                .publish(topic, QoS::AtLeastOnce, false, payload.dump())
                                .map_err(|_| Report::new(DMError::IOError))
                        }) {
                            Some(result) => result?,
                            _ => {}
                        }
                    };
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
                        self.direct_command_result = Some(Ok(response));
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

                        // if no response received for 30 seconds,notify user
                        if self.direct_command_end.is_none() && start.elapsed().as_secs() > 30 {
                            jerror!(
                                func = "App::update()",
                                event = "Reboot",
                                error = "Reboot command timeout, please try again"
                            );
                            self.direct_command_result = Some(Err(Report::new(DMError::IOError)
                                .attach_printable(format!(
                                    "No response of REBOOT command for {} seconds...",
                                    start.elapsed().as_secs()
                                ))));
                        }
                    } else {
                        jdebug!(func = "App::handle_key_event()", event = "Start Reboot",);
                        self.direct_command_request = Some(self.send_rpc_reboot());
                    }
                }
                DirectCommand::GetDirectImage => {
                    if let Some(start) = self.direct_command_start {
                        jdebug!(
                            func = "App::update()",
                            event = "GetDirectImage",
                            time = format!("{}ms", start.elapsed().as_millis())
                        );

                        // if no response received for 30 seconds,notify user
                        if self.direct_command_end.is_none() && start.elapsed().as_secs() > 30 {
                            jerror!(
                                func = "App::update()",
                                event = "DirectGetImage",
                                error = "DirectGetImage command timeout, please try again"
                            );
                            self.direct_command_result = Some(Err(Report::new(DMError::IOError)
                                .attach_printable(format!(
                                    "No response of DirectGetImage command for {} seconds...",
                                    start.elapsed().as_secs()
                                ))));
                        }
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
                        jdebug!(
                            func = "App::handle_key_event()",
                            event = "Start FactoryReset"
                        );
                        self.direct_command_request = Some(self.send_rpc_factory_reset());
                    }
                }
                _ => {}
            }
        }

        if let Ok(v) = self.rx.try_recv() {
            match v {
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
            }
        }

        // EVP agent will send state at least report_status_interval_max seconds
        // Here a threshold value in seconds of
        //    report_status_interval_max + 5
        // is used to judge whether device is disconnected.
        // That is, if no messages have been sent from device, the device is considered to be
        // disconnected.

        let report_status_interval_max = self
            .agent_device_config
            .as_ref()
            .map_or(180, |config| config.report_status_interval_max);
        let threshold = (report_status_interval_max + 5) as i64;
        if (Local::now() - self.last_connected).num_seconds() > threshold {
            self.device_connected = false;
        }

        Ok(result)
    }

    pub fn device_info(&self) -> Option<&DeviceInfo> {
        self.device_info.as_ref()
    }

    pub fn agent_system_info(&self) -> Option<&AgentSystemInfo> {
        self.agent_system_info.as_deref()
    }

    pub fn deployment_status(&self) -> Option<&DeploymentStatus> {
        self.deployment_status.as_ref()
    }

    pub fn agent_device_config(&self) -> Option<&AgentDeviceConfig> {
        self.agent_device_config.as_ref()
    }

    pub fn last_connected_time(&self) -> DateTime<Local> {
        self.last_connected
    }

    #[allow(dead_code)]
    pub fn last_connected(&self) -> String {
        self.last_connected.format("%Y-%m-%d %H:%M:%S").to_string()
    }

    pub fn device_states(&self) -> Option<&DeviceStates> {
        self.device_states.as_ref()
    }

    pub fn device_capabilities(&self) -> Option<&DeviceCapabilities> {
        self.device_capabilities.as_ref()
    }

    pub fn device_reserved(&self) -> Option<&DeviceReserved> {
        self.device_reserved.as_ref()
    }

    pub fn system_settings(&self) -> Option<&SystemSettings> {
        self.system_settings.as_ref()
    }

    pub fn network_settings(&self) -> Option<&NetworkSettings> {
        self.network_settings.as_deref()
    }

    pub fn wireless_settings(&self) -> Option<&WirelessSettings> {
        self.wireless_settings.as_ref()
    }

    #[allow(dead_code)]
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

    pub fn direct_command_request(&self) -> Option<&Result<String, DMError>> {
        self.direct_command_request.as_ref()
    }

    pub fn direct_command_result(&self) -> Option<&Result<RpcResInfo, DMError>> {
        self.direct_command_result.as_ref()
    }

    pub fn direct_command_clear(&mut self) {
        self.direct_command = None;
        self.direct_command_request = None;
        self.direct_command_result = None;
        self.direct_command_start = None;
        self.direct_command_end = None;
    }

    pub fn save_elogs(&mut self) -> Result<String, DMError> {
        if !self.elogs.is_empty() {
            let elog_path = format!("elogs_{}.json", Local::now().format("%Y%m%d_%H%M%S"));
            let mut file = std::fs::File::create(&elog_path)
                .map_err(|e| Report::new(DMError::IOError).attach_printable(e))?;
            serde_json::to_writer(&mut file, &self.elogs)
                .map_err(|e| Report::new(DMError::InvalidData).attach_printable(e))?;
            jdebug!(
                func = "MqttCtrl::save_elogs()",
                line = line!(),
                note = "elogs saved",
                elog_path = &elog_path
            );
            Ok(elog_path)
        } else {
            Err(Report::new(DMError::InvalidData).attach_printable("No elogs to save"))
        }
    }

    pub fn save_direct_get_image(&mut self) -> Result<String, DMError> {
        if let Some(Ok(response)) = &self.direct_command_result {
            if let Some(image) = &response.image {
                if image.trim().is_empty() {
                    return Err(
                        Report::new(DMError::InvalidData).attach_printable("Image data is empty")
                    );
                }

                let bytes = general_purpose::STANDARD.decode(image).map_err(|_| {
                    Report::new(DMError::InvalidData).attach_printable("DecodeError".to_string())
                })?;

                let image_path = format!(
                    "direct_get_image_{}.jpg",
                    Local::now().format("%Y%m%d_%H%M%S")
                );

                std::fs::write(&image_path, bytes)
                    .map_err(|e| Report::new(DMError::IOError).attach_printable(e))?;
                jdebug!(
                    func = "MqttCtrl::save_direct_get_image()",
                    line = line!(),
                    note = "DirectGetImage saved",
                    image_path = &image_path
                );
                return Ok(image_path);
            }
        }

        Err(Report::new(DMError::InvalidData)
            .attach_printable("No image found in direct command response"))
    }

    pub fn elogs(&self) -> &[Elog] {
        &self.elogs
    }

    pub fn firmware(&self) -> &FirmwareProperty {
        &self.firmware
    }

    #[allow(dead_code)]
    pub fn firmware_mut(&mut self) -> &mut FirmwareProperty {
        &mut self.firmware
    }

    pub fn ai_model(&self) -> &AiModel {
        &self.ai_model
    }

    #[allow(dead_code)]
    pub fn ai_model_mut(&mut self) -> &mut AiModel {
        &mut self.ai_model
    }

    pub fn edge_app_passthrough(&self) -> &EdgeAppPassthrough {
        &self.edge_app_passthrough
    }

    #[allow(dead_code)]
    pub fn edge_app_passthrough_mut(&mut self) -> &mut EdgeAppPassthrough {
        &mut self.edge_app_passthrough
    }

    pub fn load_edge_app_passthrough_config(&self, config_keys: &mut [String]) {
        use crate::app::ConfigKey;

        let edge_app = &self.edge_app_passthrough;

        // Helper function to set config value if it exists
        let set_config = |config_keys: &mut [String], key: ConfigKey, value: Option<String>| {
            if let Some(v) = value {
                if let Some(config_value) = config_keys.get_mut(usize::from(key)) {
                    *config_value = v;
                }
            }
        };

        if let Some(common_settings) = &edge_app.common_settings {
            // Common settings
            set_config(
                config_keys,
                ConfigKey::EdgeAppPassthroughProcessState,
                common_settings.process_state.map(|v| v.to_string()),
            );
            set_config(
                config_keys,
                ConfigKey::EdgeAppPassthroughLogLevel,
                common_settings.log_level.map(|v| v.to_string()),
            );

            if let Some(inference_settings) = &common_settings.inference_settings {
                set_config(
                    config_keys,
                    ConfigKey::EdgeAppPassthroughNumberOfIterations,
                    inference_settings
                        .number_of_iterations
                        .map(|v| v.to_string()),
                );
            }

            set_config(
                config_keys,
                ConfigKey::EdgeAppPassthroughNumberOfInferencePerMessage,
                common_settings
                    .number_of_inference_per_message
                    .map(|v| v.to_string()),
            );

            // PQ Settings
            if let Some(pq_settings) = &common_settings.pq_settings {
                if let Some(camera_image_size) = &pq_settings.camera_image_size {
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughCameraImageSizeWidth,
                        camera_image_size.width.map(|v| v.to_string()),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughCameraImageSizeHeight,
                        camera_image_size.height.map(|v| v.to_string()),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughCameraImageSizeScalingPolicy,
                        camera_image_size.scaling_policy.map(|v| v.to_string()),
                    );
                }

                if let Some(frame_rate) = &pq_settings.frame_rate {
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughFrameRateNum,
                        frame_rate.num.map(|v| v.to_string()),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughFrameRateDenom,
                        frame_rate.denom.map(|v| v.to_string()),
                    );
                }

                set_config(
                    config_keys,
                    ConfigKey::EdgeAppPassthroughDigitalZoom,
                    pq_settings.digital_zoom.map(|v| v.to_string()),
                );

                if let Some(camera_image_flip) = &pq_settings.camera_image_flip {
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughCameraImageFlipHorizontal,
                        camera_image_flip.flip_horizontal.map(|v| v.to_string()),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughCameraImageFlipVertical,
                        camera_image_flip.flip_vertical.map(|v| v.to_string()),
                    );
                }

                set_config(
                    config_keys,
                    ConfigKey::EdgeAppPassthroughExposureMode,
                    pq_settings.exposure_mode.map(|v| v.to_string()),
                );

                if let Some(auto_exposure) = &pq_settings.auto_exposure {
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughAutoExposureMaxTime,
                        auto_exposure.max_exposure_time.map(|v| v.to_string()),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughAutoExposureMinTime,
                        auto_exposure.min_exposure_time.map(|v| v.to_string()),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughAutoExposureMaxGain,
                        auto_exposure.max_gain.map(|v| v.to_string()),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughAutoExposureConvergenceSpeed,
                        auto_exposure.convergence_speed.map(|v| v.to_string()),
                    );
                }

                if let Some(auto_exposure_metering) = &pq_settings.auto_exposure_metering {
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughAutoExposureMeteringMode,
                        auto_exposure_metering.metering_mode.map(|v| v.to_string()),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughAutoExposureMeteringTop,
                        auto_exposure_metering.top.map(|v| v.to_string()),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughAutoExposureMeteringLeft,
                        auto_exposure_metering.left.map(|v| v.to_string()),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughAutoExposureMeteringBottom,
                        auto_exposure_metering.bottom.map(|v| v.to_string()),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughAutoExposureMeteringRight,
                        auto_exposure_metering.right.map(|v| v.to_string()),
                    );
                }

                set_config(
                    config_keys,
                    ConfigKey::EdgeAppPassthroughEvCompensation,
                    pq_settings.ev_compensation.map(|v| v.to_string()),
                );
                set_config(
                    config_keys,
                    ConfigKey::EdgeAppPassthroughAeAntiFlickerMode,
                    pq_settings.ae_anti_flicker_mode.map(|v| v.to_string()),
                );

                if let Some(manual_exposure) = &pq_settings.manual_exposure {
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughManualExposureTime,
                        manual_exposure.exposure_time.map(|v| v.to_string()),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughManualExposureGain,
                        manual_exposure.gain.map(|v| v.to_string()),
                    );
                }

                set_config(
                    config_keys,
                    ConfigKey::EdgeAppPassthroughWhiteBalanceMode,
                    pq_settings.white_balance_mode.map(|v| v.to_string()),
                );

                if let Some(auto_white_balance) = &pq_settings.auto_white_balance {
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughAutoWhiteBalanceConvergenceSpeed,
                        auto_white_balance.convergence_speed.map(|v| v.to_string()),
                    );
                }

                if let Some(manual_white_balance_preset) = &pq_settings.manual_white_balance_preset
                {
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughManualWhiteBalanceColorTemperature,
                        manual_white_balance_preset
                            .color_temperature
                            .map(|v| v.to_string()),
                    );
                }

                if let Some(image_cropping) = &pq_settings.image_cropping {
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughImageCroppingLeft,
                        image_cropping.left.map(|v| v.to_string()),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughImageCroppingTop,
                        image_cropping.top.map(|v| v.to_string()),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughImageCroppingWidth,
                        image_cropping.width.map(|v| v.to_string()),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughImageCroppingHeight,
                        image_cropping.height.map(|v| v.to_string()),
                    );
                }

                set_config(
                    config_keys,
                    ConfigKey::EdgeAppPassthroughImageRotation,
                    pq_settings.image_rotation.map(|v| v.to_string()),
                );
            }

            // Port Settings
            if let Some(port_settings) = &common_settings.port_settings {
                if let Some(metadata) = &port_settings.metadata {
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughMetadataMethod,
                        metadata.method.map(|v| v.to_string()),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughMetadataStorageName,
                        metadata.storage_name.clone(),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughMetadataEndpoint,
                        metadata.endpoint.clone(),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughMetadataPath,
                        metadata.path.clone(),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughMetadataEnabled,
                        metadata.enabled.map(|v| v.to_string()),
                    );
                }

                if let Some(input_tensor) = &port_settings.input_tensor {
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughInputTensorMethod,
                        input_tensor.method.map(|v| v.to_string()),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughInputTensorStorageName,
                        input_tensor.storage_name.clone(),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughInputTensorEndpoint,
                        input_tensor.endpoint.clone(),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughInputTensorPath,
                        input_tensor.path.clone(),
                    );
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughInputTensorEnabled,
                        input_tensor.enabled.map(|v| v.to_string()),
                    );
                }
            }

            // Codec Settings
            if let Some(codec_settings) = &common_settings.codec_settings {
                set_config(
                    config_keys,
                    ConfigKey::EdgeAppPassthroughCodecFormat,
                    codec_settings.format.map(|v| v.to_string()),
                );
            }
        }

        // Custom Settings - AI models
        if let Some(custom_settings) = &edge_app.custom_settings {
            if let Some(ai_models) = &custom_settings.ai_models {
                if let Some(passthrough_model) = ai_models.get("passthrough") {
                    set_config(
                        config_keys,
                        ConfigKey::EdgeAppPassthroughAiModelBundleId,
                        passthrough_model.ai_model_bundle_id.clone(),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_global_mqtt_ctrl_invalid_port() {
        // Passing a non-numeric port should return an error
        let res = init_global_mqtt_ctrl("broker:invalid_port");
        assert!(res.is_err());
    }

    #[test]
    #[should_panic]
    fn test_get_global_mqtt_ctrl_ref_panics_when_uninitialized() {
        // If the global OnceLock hasn't been initialized, accessing the ref should panic
        let _ = get_global_mqtt_ctrl_ref();
    }

    #[test]
    #[should_panic]
    fn test_with_mqtt_ctrl_panics_when_uninitialized() {
        // with_mqtt_ctrl expects the global to be initialized and will panic otherwise
        with_mqtt_ctrl(|_c| {});
    }

    #[test]
    #[should_panic]
    fn test_with_mqtt_ctrl_mut_panics_when_uninitialized() {
        // with_mqtt_ctrl_mut expects the global to be initialized and will panic otherwise
        with_mqtt_ctrl_mut(|_c| {});
    }
}
