#[allow(unused)]
use {
    super::{
        JsonUtility,
        device_info::{
            DeviceCapabilities, DeviceInfo, DeviceReserved, DeviceStates, NetworkSettings,
            SystemSettings, WirelessSettings,
        },
        evp_state::UUID,
        evp_state::{AgentDeviceConfig, AgentSystemInfo},
    },
    super::{ReqId, ResInfo},
    crate::mqtt_ctrl::MqttCtrl,
    crate::mqtt_ctrl::evp::evp_state::UUID as EvpUUID,
    crate::{
        app::{App, ConfigKey},
        error::{DMError, DMErrorExt},
    },
    error_stack::{Context, Report, Result, ResultExt},
    jlogger_tracing::{JloggerBuilder, LevelFilter, LogTimeFormat, jdebug, jerror, jinfo},
    json::{JsonValue, object::Object},
    pest::{Parser, Token},
    regex::Regex,
    rumqttc::{Client, Connection, MqttOptions, QoS},
    serde::{Deserialize, Serialize},
    std::fmt::Display,
    std::{
        collections::HashMap,
        time::{self, Duration, Instant},
    },
};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
pub struct InferenceSettings {
    /// Number of interactions
    number_of_iterations: Option<u32>,
}

impl InferenceSettings {
    pub fn number_of_iterations(&self) -> Option<u32> {
        self.number_of_iterations
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
pub struct CameraImageSize {
    /// Width of the camera image
    width: Option<u32>,

    /// Height of the camera image
    height: Option<u32>,

    /// This determines which factory is prioritized when scaling is necessary to achieve the desired image size.
    /// 1: sensitivity, 2: resolution
    scaling_policy: Option<i8>,
}

impl Display for CameraImageSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let width = self.width.map(|a| a.to_string()).unwrap_or("?".to_string());
        let height = self
            .height
            .map(|a| a.to_string())
            .unwrap_or("?".to_string());
        let scaling_policy = self
            .scaling_policy
            .map(|a| match a {
                1 => "sensitivity".to_owned(),
                2 => "resolution".to_owned(),
                _ => format!("invalid: {}", a),
            })
            .unwrap_or("?".to_string());

        write!(
            f,
            "{}x{}, scaling_policy: {}",
            width, height, scaling_policy
        )
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
pub struct FrameRate {
    /// Numerator
    num: Option<i32>,
    /// Denominator
    denom: Option<i32>,
}

impl Display for FrameRate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let num = self.num.map(|a| a.to_string()).unwrap_or("?".to_string());
        let denom = self.denom.map(|a| a.to_string()).unwrap_or("?".to_string());

        write!(f, "{}/{}", num, denom)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
pub struct CameraImageFlip {
    /// Horizontal flip: 0: normal, 1: flip
    flip_horizontal: Option<i8>,

    /// Vertical flip: 0: normal, 1: flip
    flip_vertical: Option<i8>,
}

impl Display for CameraImageFlip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let flip_horizontal = self
            .flip_horizontal
            .map(|a| if a == 1 { "h_flip" } else { "h_normal" })
            .unwrap_or("?");
        let flip_vertical = self
            .flip_vertical
            .map(|a| if a == 1 { "v_flip" } else { "v_normal" })
            .unwrap_or("?");

        write!(f, "{}, {}", flip_horizontal, flip_vertical)
    }
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct AutoExposure {
    /// The maximum time in microseconds that the auto exposure keeps the shutter open, up the period based on the 'frameRate".
    max_exposure_time: Option<i32>,

    /// The minimum time in microseconds that the auto exposure keeps the shutter open, this must be less than or equal to the 'max_exposure_time'.
    min_exposure_time: Option<i32>,

    /// The maximum gain value in dB that the auto exposure sets.
    max_gain: Option<f32>,

    /// Convergence speed.
    convergence_speed: Option<i32>,
}

impl AutoExposure {
    pub fn max_exposure_time(&self) -> Option<i32> {
        self.max_exposure_time
    }

    pub fn min_exposure_time(&self) -> Option<i32> {
        self.min_exposure_time
    }

    pub fn max_gain(&self) -> Option<f32> {
        self.max_gain
    }

    pub fn convergence_speed(&self) -> Option<i32> {
        self.convergence_speed
    }
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct ManualExposure {
    /// The time in microseconds that the shutter is kept open.
    exposure_time: Option<i32>,

    /// The gain value in dB.
    gain: Option<f32>,
}

impl ManualExposure {
    pub fn exposure_time(&self) -> Option<i32> {
        self.exposure_time
    }

    pub fn gain(&self) -> Option<f32> {
        self.gain
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
pub struct AutoWhiteBalance {
    /// The convergence frame number for changing from 4300K to 5600K.
    convergence_speed: Option<i32>,
}

impl AutoWhiteBalance {
    pub fn convergence_speed(&self) -> Option<i32> {
        self.convergence_speed
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
pub struct ManualWhiteBalancePreset {
    /// Color temperature: 0: 3200K, 1: 4300K, 2: 5600K, 3: 6500K
    color_temperature: Option<i8>,
}

impl ManualWhiteBalancePreset {
    pub fn color_temperature(&self) -> Option<i8> {
        self.color_temperature
    }
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct ManualWhiteBalanceGain {
    red: Option<f32>,
    blue: Option<f32>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
pub struct ImageCropping {
    left: Option<i32>,
    top: Option<i32>,
    width: Option<u32>,
    height: Option<u32>,
}

impl Display for ImageCropping {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let left = self.left.map(|a| a.to_string()).unwrap_or("?".to_string());
        let top = self.top.map(|a| a.to_string()).unwrap_or("?".to_string());
        let width = self.width.map(|a| a.to_string()).unwrap_or("?".to_string());
        let height = self
            .height
            .map(|a| a.to_string())
            .unwrap_or("?".to_string());

        write!(f, "{}x{}@({},{})", width, height, left, top)
    }
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct RegisterAccess {
    register: Option<u32>,
    value: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct PQSettings {
    /// The size of the camera images, which is also used as the coordinate for transformation operations.
    camera_image_size: Option<CameraImageSize>,

    /// The frame rate at which the sensor outputs images.
    frame_rate: Option<FrameRate>,

    /// The magnification value for digital zooming, which affects the output images.
    digital_zoom: Option<f32>,

    /// Camera image flip.
    camera_image_flip: Option<CameraImageFlip>,

    /// Exposure mode: 0: auto, 1: manual
    exposure_mode: Option<i8>,

    /// Those values are interpreted only when the 'exposureMode' is set to 'auto'.
    auto_exposure: Option<AutoExposure>,

    /// The exposure compensation value. This value is interpreted only when the 'exposureMode' is set to 'auto'.
    ev_compensation: Option<f32>,

    /// This enables the anti-flickering functionality of the auto exposure.
    /// 0: off, 1: auto, 2: freq_50Hz, 3: freq_60Hz
    ae_anti_flicker_mode: Option<i8>,

    /// Those values are interpreted only when the 'exposureMode' is set to 'manual'.
    manual_exposure: Option<ManualExposure>,

    /// White balance mode: 0: auto, 1: preset
    white_balance_mode: Option<i8>,

    /// This value is interpreted only when the 'whiteBalanceMode' is set to 'auto'.
    auto_white_balance: Option<AutoWhiteBalance>,

    /// This value is interpreted only when the 'whiteBalanceMode' is set to 'preset'.
    manual_white_balance_preset: Option<ManualWhiteBalancePreset>,

    /// Manual white balance gain (DEPRECATED)
    manual_white_balance_gain: Option<ManualWhiteBalanceGain>,

    /// The cropping boundary used to generate input tensor images for the IMX500 DNN processor. The coordinate is based on the 'ImageSize'.
    image_cropping: Option<ImageCropping>,

    /// The rotation angle of the input tensor images for the IMX500 DNN processor.
    ///   0: none, 1: clockwise 90 degrees, 2: clockwise 180 degrees, 3: clockwise 270 degrees
    image_rotation: Option<i8>,

    register_access: Option<Vec<RegisterAccess>>,
}

impl PQSettings {
    pub fn camera_image_size(&self) -> Option<&CameraImageSize> {
        self.camera_image_size.as_ref()
    }

    pub fn frame_rate(&self) -> Option<&FrameRate> {
        self.frame_rate.as_ref()
    }

    pub fn digital_zoom(&self) -> Option<f32> {
        self.digital_zoom
    }

    pub fn camera_image_flip(&self) -> Option<&CameraImageFlip> {
        self.camera_image_flip.as_ref()
    }

    pub fn exposure_mode(&self) -> Option<i8> {
        self.exposure_mode
    }

    pub fn auto_exposure(&self) -> Option<&AutoExposure> {
        self.auto_exposure.as_ref()
    }

    pub fn ev_compensation(&self) -> Option<f32> {
        self.ev_compensation
    }

    pub fn ae_anti_flicker_mode(&self) -> Option<i8> {
        self.ae_anti_flicker_mode
    }

    pub fn manual_exposure(&self) -> Option<&ManualExposure> {
        self.manual_exposure.as_ref()
    }

    pub fn white_balance_mode(&self) -> Option<i8> {
        self.white_balance_mode
    }

    pub fn auto_white_balance(&self) -> Option<&AutoWhiteBalance> {
        self.auto_white_balance.as_ref()
    }

    pub fn manual_white_balance_preset(&self) -> Option<&ManualWhiteBalancePreset> {
        self.manual_white_balance_preset.as_ref()
    }

    pub fn manual_white_balance_gain(&self) -> Option<&ManualWhiteBalanceGain> {
        self.manual_white_balance_gain.as_ref()
    }

    pub fn image_cropping(&self) -> Option<&ImageCropping> {
        self.image_cropping.as_ref()
    }

    pub fn image_rotation(&self) -> Option<i8> {
        self.image_rotation
    }

    pub fn register_access(&self) -> Option<&Vec<RegisterAccess>> {
        self.register_access.as_ref()
    }
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct DataInterface {
    /// Method: 0: evp telemetry, 1: blob storage, 2: http storage
    method: Option<i8>,

    /// This is used when the method is set to 'blob storage'.
    storage_name: Option<String>,

    /// This is required when the method is set to 'http storage'.
    endpoint: Option<String>,

    /// This is required when the method is set to 'http storage' or 'blob storage'.
    path: Option<String>,

    /// Enabled
    enabled: Option<bool>,
}

impl DataInterface {
    pub fn method(&self) -> Option<i8> {
        self.method
    }

    pub fn storage_name(&self) -> Option<&String> {
        self.storage_name.as_ref()
    }

    pub fn endpoint(&self) -> Option<&String> {
        self.endpoint.as_ref()
    }

    pub fn path(&self) -> Option<&String> {
        self.path.as_ref()
    }

    pub fn enabled(&self) -> Option<bool> {
        self.enabled
    }
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct PortSettings {
    /// Interface for sending metadata.
    metadata: Option<DataInterface>,

    /// Interface for sending input tensor images.
    input_tensor: Option<DataInterface>,
}

impl PortSettings {
    pub fn metadata(&self) -> Option<&DataInterface> {
        self.metadata.as_ref()
    }

    pub fn input_tensor(&self) -> Option<&DataInterface> {
        self.input_tensor.as_ref()
    }
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct CodecSettings {
    /// Format: 1: JPEG
    format: Option<i8>,
}

impl CodecSettings {
    pub fn format(&self) -> Option<i8> {
        self.format
    }
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct CommonSettings {
    /// Process state 1: Stopped, 2: Running
    process_state: Option<i8>,

    /// Log level 0:trace, 1:debug, 2:info, 3:warn, 4:error, 5:critical
    log_level: Option<i8>,

    /// Inference settings
    inference_settings: Option<InferenceSettings>,

    /// PQ settings to set from cloud and to use in device.
    pq_settings: Option<PQSettings>,

    /// Transport settings defined by EdgeApp Developer.
    port_settings: Option<PortSettings>,

    /// Codec settings
    codec_settings: Option<CodecSettings>,

    /// Number of inference per message.
    number_of_inference_per_message: Option<i32>,

    /// Upload interval
    upload_interval: Option<i32>,
}

impl CommonSettings {
    pub fn process_state(&self) -> Option<i8> {
        self.process_state
    }

    pub fn log_level(&self) -> Option<i8> {
        self.log_level
    }

    pub fn inference_settings(&self) -> Option<&InferenceSettings> {
        self.inference_settings.as_ref()
    }

    pub fn pq_settings(&self) -> Option<&PQSettings> {
        self.pq_settings.as_ref()
    }

    pub fn port_settings(&self) -> Option<&PortSettings> {
        self.port_settings.as_ref()
    }

    pub fn codec_settings(&self) -> Option<&CodecSettings> {
        self.codec_settings.as_ref()
    }

    pub fn number_of_inference_per_message(&self) -> Option<i32> {
        self.number_of_inference_per_message
    }

    pub fn upload_interval(&self) -> Option<i32> {
        self.upload_interval
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct CustomSettingsPassthrough {
    ai_model_bundle_id: String,
}

#[derive(Debug, Default, PartialEq)]
pub struct CustomSettingsDetectionParameters {
    max_detections: u32,
    threshold: f32,
    input_width: u32,
    input_height: u32,
    bbox_order: String,
    bbox_normalization: bool,
    class_score_order: String,
}

#[derive(Debug, Default, PartialEq)]
pub struct CustomSettingsDetection {
    ai_model_bundle_id: String,
    parameters: CustomSettingsDetectionParameters,
}

#[derive(Debug, Default, PartialEq)]
pub struct CustomSettingsMetaSettings {
    format: i8,
}

#[derive(Debug, Default, PartialEq)]
pub struct CustomSettings {
    res_info: Option<ResInfo>,
    ai_model_passthrough: Option<CustomSettingsPassthrough>,
    ai_model_detection: Option<CustomSettingsDetection>,
    metadata_settings: Option<CustomSettingsMetaSettings>,
    custom: Option<String>,
}

impl CustomSettings {
    pub fn res_info(&self) -> Option<&ResInfo> {
        self.res_info.as_ref()
    }

    pub fn ai_model_passthrough(&self) -> Option<&CustomSettingsPassthrough> {
        self.ai_model_passthrough.as_ref()
    }

    pub fn ai_model_detection(&self) -> Option<&CustomSettingsDetection> {
        self.ai_model_detection.as_ref()
    }

    pub fn metadata_settings(&self) -> Option<&CustomSettingsMetaSettings> {
        self.metadata_settings.as_ref()
    }

    pub fn custom(&self) -> Option<&String> {
        self.custom.as_ref()
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct EdgeApp {
    req_info: Option<ReqId>,
    common_settings: CommonSettings,
    custom_settings: Option<CustomSettings>,

    res_info: Option<ResInfo>,
}

impl EdgeApp {
    pub fn req_info(&self) -> Option<&ReqId> {
        self.req_info.as_ref()
    }

    pub fn res_info(&self) -> Option<&ResInfo> {
        self.res_info.as_ref()
    }

    pub fn common_settings(&self) -> &CommonSettings {
        &self.common_settings
    }

    pub fn custom_settings(&self) -> Option<&CustomSettings> {
        self.custom_settings.as_ref()
    }

    pub fn parse(payload: &str) -> Result<Self, DMError> {
        jdebug!(func = "EdgeApp::parse()", line = line!(), payload = payload);

        let json = json::parse(payload).map_err(|e| {
            Report::new(DMError::InvalidData)
                .attach_printable(format!("Failed to parse JSON: {}", e))
                .change_context(DMError::InvalidData)
        })?;

        let mut res_info: Option<ResInfo> = None;
        let mut req_info: Option<ReqId> = None;
        let mut common_settings: Option<CommonSettings> = None;
        let mut custom_settings: Option<CustomSettings> = None;
        if let JsonValue::Object(o) = json {
            for (key, value) in o.iter() {
                jdebug!(
                    func = "EdgeApp::parse()",
                    line = line!(),
                    key = key,
                    value = value.dump()
                );

                match key {
                    "res_info" => {
                        jdebug!(
                            func = "EdgeApp::parse()",
                            line = line!(),
                            key = key,
                            value = value.dump()
                        );
                        res_info = Some(
                            serde_json::from_str(&JsonUtility::json_value_to_string(value))
                                .map_err(|e| {
                                    Report::new(DMError::InvalidData).attach_printable(e)
                                })?,
                        );
                    }
                    "req_info" => {
                        jdebug!(
                            func = "EdgeApp::parse()",
                            line = line!(),
                            key = key,
                            value = value.dump()
                        );
                        req_info = Some(
                            serde_json::from_str(&JsonUtility::json_value_to_string(value))
                                .map_err(|e| {
                                    Report::new(DMError::InvalidData).attach_printable(e)
                                })?,
                        );
                    }
                    "common_settings" => {
                        jdebug!(
                            func = "EdgeApp::parse()",
                            line = line!(),
                            key = key,
                            value = value.dump()
                        );
                        common_settings = Some(
                            serde_json::from_str(&JsonUtility::json_value_to_string(value))
                                .map_err(|e| {
                                    Report::new(DMError::InvalidData).attach_printable(e)
                                })?,
                        );
                    }
                    "custom_settings" => {
                        jdebug!(
                            func = "EdgeApp::parse()",
                            line = line!(),
                            key = key,
                            value = value.dump()
                        );
                        custom_settings = Some(CustomSettings {
                            custom: Some(JsonUtility::json_value_to_string(value)),
                            ..Default::default()
                        })
                    }
                    _ => {
                        return Err(Report::new(DMError::InvalidData)
                            .attach_printable(format!("Unknown key in JSON: {}", key)));
                    }
                }
            }

            return Ok(Self {
                req_info,
                common_settings: common_settings.ok_or(Report::new(DMError::InvalidData))?,
                custom_settings,
                res_info,
            });
        }

        Err(Report::new(DMError::InvalidData).attach_printable(format!("Invalid Json: {payload}")))
    }
}

#[derive(pest_derive::Parser)]
#[grammar = "src/mqtt_ctrl/evp/evp.pest"]
struct EvpParser;

#[derive(Debug, Default, PartialEq)]
pub struct EdgeAppInfo {
    id: String,
    module: EdgeApp,
}

#[allow(unused)]
impl EdgeAppInfo {
    pub fn parse(key: &str, payload: &str) -> Result<Self, DMError> {
        let pairs = EvpParser::parse(Rule::edge_app, key)
            .map_err(|e| Report::new(DMError::InvalidData).attach_printable(e))?;
        let mut id_start = 0;
        let mut id_end = 0;

        for token in pairs.tokens() {
            match token {
                Token::Start { rule, pos } => {
                    if rule == Rule::uuid {
                        id_start = pos.pos()
                    }
                }

                Token::End { rule, pos } => {
                    if rule == Rule::uuid {
                        id_end = pos.pos()
                    }
                }
            }
        }

        let id = key[id_start..id_end].to_string();

        let module = EdgeApp::parse(payload).map_err(|e| {
            Report::new(DMError::InvalidData)
                .attach_printable(format!("Failed to parse EdgeApp: {}", e))
        })?;

        Ok(Self { id, module })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn module(&self) -> &EdgeApp {
        &self.module
    }

    pub fn parse_configure(&self, config_keys: &Vec<String>) -> Result<String, DMError> {
        let mut edge_app = Object::new();
        enum EntryType {
            NumericI8,
            NumericI32,
            NumericUsize,
            NumericU32,
            NumericFloat,
            StringType,
            BoolType,
        }

        let mut json_entry = |root: &mut Object,
                              config_key: usize,
                              key: &str,
                              t: EntryType|
         -> Result<(), DMError> {
            let value = config_keys.get(config_key).unwrap();
            if !value.is_empty() {
                match t {
                    EntryType::NumericI8 => {
                        let value_number: i8 = value.parse().map_err(|e| {
                            Report::new(DMError::InvalidData)
                                .attach_printable(format!("Invalid {}: {e}", value))
                        })?;
                        root.insert(key, JsonValue::Number(value_number.into()));
                    }
                    EntryType::NumericU32 => {
                        let value_number: u32 = value.parse().map_err(|e| {
                            Report::new(DMError::InvalidData)
                                .attach_printable(format!("Invalid {}: {e}", value))
                        })?;
                        root.insert(key, JsonValue::Number(value_number.into()));
                    }
                    EntryType::NumericI32 => {
                        let value_number: i32 = value.parse().map_err(|e| {
                            Report::new(DMError::InvalidData)
                                .attach_printable(format!("Invalid {}: {e}", value))
                        })?;
                        root.insert(key, JsonValue::Number(value_number.into()));
                    }
                    EntryType::NumericFloat => {
                        let value_number: f32 = value.parse().map_err(|e| {
                            Report::new(DMError::InvalidData)
                                .attach_printable(format!("Invalid {}: {e}", value))
                        })?;
                        root.insert(key, JsonValue::Number(value_number.into()));
                    }
                    EntryType::NumericUsize => {
                        let value_number: usize = value.parse().map_err(|e| {
                            Report::new(DMError::InvalidData)
                                .attach_printable(format!("Invalid {}: {e}", value))
                        })?;
                        root.insert(key, JsonValue::Number(value_number.into()));
                    }
                    EntryType::StringType => {
                        root.insert(key, JsonValue::String(value.to_string()));
                    }
                    EntryType::BoolType => {
                        let value_bool: bool = value.parse().map_err(|e| {
                            Report::new(DMError::InvalidData)
                                .attach_printable(format!("Invalid {}: {e}", value))
                        })?;
                        root.insert(key, JsonValue::Boolean(value_bool));
                    }
                }
            }
            Ok(())
        };

        // Req info
        {
            let mut req_info = Object::new();
            let uuid = EvpUUID::new();

            req_info.insert("req_id", JsonValue::String(uuid.uuid().to_string()));
            edge_app.insert("req_info", JsonValue::Object(req_info));
        }

        // Common settings
        {
            let mut common_settings = Object::new();

            // Process state
            json_entry(
                &mut common_settings,
                ConfigKey::CommonSettingsProcessState as usize,
                "process_state",
                EntryType::NumericI8,
            )?;

            // Log level
            json_entry(
                &mut common_settings,
                ConfigKey::CommonSettingsLogLevel as usize,
                "log_level",
                EntryType::NumericI8,
            )?;

            // Inference_settings
            {
                let mut inference_settings = Object::new();

                json_entry(
                    &mut inference_settings,
                    ConfigKey::CommonSettingsISNumberOfIterations as usize,
                    "number_of_iterations",
                    EntryType::NumericU32,
                )?;

                if !inference_settings.is_empty() {
                    common_settings
                        .insert("inference_settings", JsonValue::Object(inference_settings));
                }
            }

            // PQ settings
            {
                let mut pq_settings = Object::new();

                // Camera image size
                {
                    let mut camera_image_size = Object::new();
                    json_entry(
                        &mut camera_image_size,
                        ConfigKey::CommonSettingsPQCameraImageSizeWidth as usize,
                        "width",
                        EntryType::NumericU32,
                    )?;

                    json_entry(
                        &mut camera_image_size,
                        ConfigKey::CommonSettingsPQCameraImageSizeHeight as usize,
                        "height",
                        EntryType::NumericU32,
                    )?;

                    json_entry(
                        &mut camera_image_size,
                        ConfigKey::CommonSettingsPQCameraImageSizeScalingPolicy as usize,
                        "scaling_policy",
                        EntryType::NumericI8,
                    )?;

                    if !camera_image_size.is_empty() {
                        pq_settings
                            .insert("camera_image_size", JsonValue::Object(camera_image_size));
                    }
                }

                // Frame rate
                {
                    let mut frame_rate = Object::new();
                    json_entry(
                        &mut frame_rate,
                        ConfigKey::CommonSettingsPQFrameRateNum as usize,
                        "num",
                        EntryType::NumericI32,
                    )?;

                    json_entry(
                        &mut frame_rate,
                        ConfigKey::CommonSettingsPQFrameRateDenom as usize,
                        "denom",
                        EntryType::NumericI32,
                    )?;

                    if !frame_rate.is_empty() {
                        pq_settings.insert("frame_rate", JsonValue::Object(frame_rate));
                    }
                }

                // Digital zoom
                json_entry(
                    &mut pq_settings,
                    ConfigKey::CommonSettingsPQDigitalZoom as usize,
                    "digital_zoom",
                    EntryType::NumericFloat,
                )?;

                // Camera image flip
                {
                    let mut camera_image_flip = Object::new();
                    json_entry(
                        &mut camera_image_flip,
                        ConfigKey::CommonSettingsPQCameraImageFlipHorizontal as usize,
                        "flip_horizontal",
                        EntryType::NumericI8,
                    )?;

                    json_entry(
                        &mut camera_image_flip,
                        ConfigKey::CommonSettingsPQCameraImageFlipVertical as usize,
                        "flip_vertical",
                        EntryType::NumericI8,
                    )?;

                    if !camera_image_flip.is_empty() {
                        pq_settings
                            .insert("camera_image_flip", JsonValue::Object(camera_image_flip));
                    }
                }

                // Exposure mode
                json_entry(
                    &mut pq_settings,
                    ConfigKey::CommonSettingsPQExposureMode as usize,
                    "exposure_mode",
                    EntryType::NumericI8,
                )?;

                // Auto exposure
                {
                    let mut auto_exposure = Object::new();
                    json_entry(
                        &mut auto_exposure,
                        ConfigKey::CommonSettingsPQAeMaxExposureTime as usize,
                        "max_exposure_time",
                        EntryType::NumericI32,
                    )?;

                    json_entry(
                        &mut auto_exposure,
                        ConfigKey::CommonSettingsPQAeMinExposureTime as usize,
                        "min_exposure_time",
                        EntryType::NumericI32,
                    )?;

                    json_entry(
                        &mut auto_exposure,
                        ConfigKey::CommonSettingsPQAeMaxGain as usize,
                        "max_gain",
                        EntryType::NumericFloat,
                    )?;

                    json_entry(
                        &mut auto_exposure,
                        ConfigKey::CommonSettingsPQAeConvergenceSpeed as usize,
                        "convergence_speed",
                        EntryType::NumericI32,
                    )?;

                    if !auto_exposure.is_empty() {
                        pq_settings.insert("auto_exposure", JsonValue::Object(auto_exposure));
                    }
                }

                // ev_compensation
                json_entry(
                    &mut pq_settings,
                    ConfigKey::CommonSettingsPQEvCompensation as usize,
                    "ev_compensation",
                    EntryType::NumericFloat,
                )?;

                // ae_anti_flicker_mode
                json_entry(
                    &mut pq_settings,
                    ConfigKey::CommonSettingsPQAeAntiFlickerMode as usize,
                    "ae_anti_flicker_mode",
                    EntryType::NumericI8,
                )?;

                // Manual exposure
                {
                    let mut manual_exposure = Object::new();
                    json_entry(
                        &mut manual_exposure,
                        ConfigKey::CommonSettingsPQMeExposureTime as usize,
                        "exposure_time",
                        EntryType::NumericI32,
                    )?;

                    json_entry(
                        &mut manual_exposure,
                        ConfigKey::CommonSettingsPQMeGain as usize,
                        "gain",
                        EntryType::NumericFloat,
                    )?;

                    if !manual_exposure.is_empty() {
                        pq_settings.insert("manual_exposure", JsonValue::Object(manual_exposure));
                    }
                }

                // White balance mode
                json_entry(
                    &mut pq_settings,
                    ConfigKey::CommonSettingsPQWhiteBalanceMode as usize,
                    "white_balance_mode",
                    EntryType::NumericI8,
                )?;

                // Auto white balance
                {
                    let mut auto_white_balance = Object::new();
                    json_entry(
                        &mut auto_white_balance,
                        ConfigKey::CommonSettingsPQAwbConvergenceSpeed as usize,
                        "convergence_speed",
                        EntryType::NumericI32,
                    )?;

                    if !auto_white_balance.is_empty() {
                        pq_settings
                            .insert("auto_white_balance", JsonValue::Object(auto_white_balance));
                    }
                }

                // Manual white balance preset
                {
                    let mut manual_white_balance_preset = Object::new();

                    json_entry(
                        &mut manual_white_balance_preset,
                        ConfigKey::CommonSettingsPQMWBPColorTemperature as usize,
                        "color_temperature",
                        EntryType::NumericI8,
                    )?;

                    if !manual_white_balance_preset.is_empty() {
                        pq_settings.insert(
                            "manual_white_balance_preset",
                            JsonValue::Object(manual_white_balance_preset),
                        );
                    }
                }

                // Manual white balance gain
                {
                    let mut manual_white_balance_gain = Object::new();
                    json_entry(
                        &mut manual_white_balance_gain,
                        ConfigKey::CommonSettingsPQMWBGRed as usize,
                        "red",
                        EntryType::NumericFloat,
                    )?;

                    json_entry(
                        &mut manual_white_balance_gain,
                        ConfigKey::CommonSettingsPQMWBGBlue as usize,
                        "blue",
                        EntryType::NumericFloat,
                    )?;

                    if !manual_white_balance_gain.is_empty() {
                        pq_settings.insert(
                            "manual_white_balance_gain",
                            JsonValue::Object(manual_white_balance_gain),
                        );
                    }
                }

                // Image cropping
                {
                    let mut image_cropping = Object::new();
                    json_entry(
                        &mut image_cropping,
                        ConfigKey::CommonSettingsPQICLeft as usize,
                        "left",
                        EntryType::NumericI32,
                    )?;

                    json_entry(
                        &mut image_cropping,
                        ConfigKey::CommonSettingsPQICTop as usize,
                        "top",
                        EntryType::NumericI32,
                    )?;

                    json_entry(
                        &mut image_cropping,
                        ConfigKey::CommonSettingsPQICWidth as usize,
                        "width",
                        EntryType::NumericU32,
                    )?;

                    json_entry(
                        &mut image_cropping,
                        ConfigKey::CommonSettingsPQICHeight as usize,
                        "height",
                        EntryType::NumericU32,
                    )?;

                    if !image_cropping.is_empty() {
                        pq_settings.insert("image_cropping", JsonValue::Object(image_cropping));
                    }
                }

                // Image rotation
                json_entry(
                    &mut pq_settings,
                    ConfigKey::CommonSettingsPQImageRotation as usize,
                    "image_rotation",
                    EntryType::NumericI8,
                )?;

                if !pq_settings.is_empty() {
                    common_settings.insert("pq_settings", JsonValue::Object(pq_settings));
                }
            }

            // Port settings
            {
                let mut port_settings = Object::new();

                // Metadata
                {
                    let mut metadata = Object::new();

                    json_entry(
                        &mut metadata,
                        ConfigKey::CommonSettingsPSMetadataMethod as usize,
                        "method",
                        EntryType::NumericI8,
                    )?;

                    json_entry(
                        &mut metadata,
                        ConfigKey::CommonSettingsPSMetadataStorageName as usize,
                        "storage_name",
                        EntryType::StringType,
                    )?;

                    json_entry(
                        &mut metadata,
                        ConfigKey::CommonSettingsPSMetadataEndpoint as usize,
                        "endpoint",
                        EntryType::StringType,
                    )?;

                    json_entry(
                        &mut metadata,
                        ConfigKey::CommonSettingsPSMetadataPath as usize,
                        "path",
                        EntryType::StringType,
                    )?;

                    json_entry(
                        &mut metadata,
                        ConfigKey::CommonSettingsPSMetadataEnabled as usize,
                        "enabled",
                        EntryType::BoolType,
                    )?;

                    if !metadata.is_empty() {
                        port_settings.insert("metadata", JsonValue::Object(metadata));
                    }
                }

                // Input tensor
                {
                    let mut input_tensor = Object::new();

                    json_entry(
                        &mut input_tensor,
                        ConfigKey::CommonSettingsPSITMethod as usize,
                        "method",
                        EntryType::NumericI8,
                    )?;

                    json_entry(
                        &mut input_tensor,
                        ConfigKey::CommonSettingsPSITStorageName as usize,
                        "storage_name",
                        EntryType::StringType,
                    )?;

                    json_entry(
                        &mut input_tensor,
                        ConfigKey::CommonSettingsPSITEndpoint as usize,
                        "endpoint",
                        EntryType::StringType,
                    )?;

                    json_entry(
                        &mut input_tensor,
                        ConfigKey::CommonSettingsPSITPath as usize,
                        "path",
                        EntryType::StringType,
                    )?;

                    json_entry(
                        &mut input_tensor,
                        ConfigKey::CommonSettingsPSITEnabled as usize,
                        "enabled",
                        EntryType::BoolType,
                    )?;

                    if !input_tensor.is_empty() {
                        port_settings.insert("input_tensor", JsonValue::Object(input_tensor));
                    }
                }

                if !port_settings.is_empty() {
                    common_settings.insert("port_settings", JsonValue::Object(port_settings));
                }
            }

            // Codec settings
            {
                let mut codec_settings = Object::new();

                json_entry(
                    &mut codec_settings,
                    ConfigKey::CommonSettingsCSFormat as usize,
                    "format",
                    EntryType::NumericI8,
                )?;

                if !codec_settings.is_empty() {
                    common_settings.insert("codec_settings", JsonValue::Object(codec_settings));
                }
            }

            // Number of inference per message
            json_entry(
                &mut common_settings,
                ConfigKey::CommonSettingsNumberOfInferencePerMessage as usize,
                "number_of_inference_per_message",
                EntryType::NumericI32,
            )?;

            // Upload interval
            json_entry(
                &mut common_settings,
                ConfigKey::CommonSettingsUploadInterval as usize,
                "upload_interval",
                EntryType::NumericI32,
            )?;

            if !common_settings.is_empty() {
                edge_app.insert("common_settings", JsonValue::Object(common_settings));
            }
        }

        // Custom settings
        if let Ok(custom_settings) = std::fs::read_to_string(format!(
            "{}/edge_app_custom_settings.json",
            App::config_dir()
        )) {
            let custom_settings = json::parse(&custom_settings)
                .map_err(|e| Report::new(DMError::InvalidData).attach_printable(e))?;
            edge_app.insert("custom_settings", custom_settings);
        }

        let mut root = Object::new();
        let key = format!("configure/{}/edge_app", self.id);
        root.insert(&key, JsonValue::Object(edge_app));

        Ok(JsonUtility::json_value_to_string(&JsonValue::Object(root)))
    }
}

mod tests {
    #[allow(unused_imports)]
    use crate::mqtt_ctrl::evp::edge_app::EdgeApp;

    #[test]
    fn test_edge_app_parse_01() {
        let json_str = r#"
        {
            "req_info": {"req_id": "12345"},
            "common_settings": {
                "process_state": 2,
                "log_level": 1,
                "inference_settings": {"number_of_iterations": 10},
                "pq_settings": {},
                "port_settings": {},
                "codec_settings": {},
                "number_of_inference_per_message": 5,
                "upload_interval": 30
            },
            "custom_settings": {
                "custom": "{\"key\":\"value\"}"
            }
        }"#;

        let edge_app = EdgeApp::parse(json_str).unwrap();
        assert_eq!(edge_app.req_info.unwrap().req_id, "12345");
        assert_eq!(edge_app.common_settings.process_state, Some(2));
        assert_eq!(
            edge_app.custom_settings.unwrap().custom,
            Some("{\"custom\":\"{\\\"key\\\":\\\"value\\\"}\"}".to_string())
        );
    }

    #[test]
    fn test_edge_app_parse_02() {
        let json_str = r#"
        {
            "res_info":{
                "code":0,
                "res_id":"",
                "detail_msg":""
            },
            "common_settings":{
                "process_state":1,
                "log_level":2,
                "inference_settings":{
                    "number_of_iterations":0
                },
                "pq_settings":{
                    "camera_image_size":{},
                    "camera_image_flip":{},
                    "digital_zoom":null,
                    "exposure_mode":null,
                    "auto_exposure":{},
                    "auto_exposure_metering":{},
                    "ev_compensation":null,
                    "ae_anti_flicker_mode":null,
                    "manual_exposure":{},
                    "frame_rate":{},
                    "white_balance_mode":null,
                    "auto_white_balance":{},
                    "manual_white_balance_preset":{},
                    "image_cropping":{},
                    "image_rotation":null,
                    "register_access":[]
                },
                "port_settings":{
                    "metadata":{},
                    "input_tensor":{}
                },
                "codec_settings":{
                    "format":1
                },
                "number_of_inference_per_message":1
            },
            "custom_settings":{}
        }"#;
        let _edge_app = EdgeApp::parse(json_str).unwrap();
    }
}
