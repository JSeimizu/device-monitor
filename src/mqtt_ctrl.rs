#[allow(unused)]
use {
    super::error::DMError,
    error_stack::{Report, Result},
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
    last_connected: Instant,
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

    pub fn update(&mut self) -> Result<HashMap<String, String>, DMError> {
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
                                .map_err(|_e| Report::new(DMError::InvalidData))?;

                            result.extend(self.process_agent_request(&topic, &payload)?);
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
            Err(_e) => {
                jdebug!(
                    func = "MqttCtrl::read()",
                    line = line!(),
                    error = format!("RecvError")
                );
            }
        }

        // If there is no messages from device for 5 minutes
        // device is considered to be disconnected.
        if self.last_connected.elapsed() > Duration::from_secs(5 * 60) {
            self.device_connected = false;
        }

        Ok(result)
    }
}
