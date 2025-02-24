mod error;

use actix_web::App;
use actix_web::HttpResponse;
use actix_web::HttpServer;
use actix_web::Responder;
use actix_web::web;
use clap::Parser;
use error::DMError;
use error_stack::{Report, Result};
use jlogger_tracing::{JloggerBuilder, LevelFilter, LogTimeFormat, jdebug, jinfo};
use regex::Regex;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde_derive::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct Item {
    id: u32,
    name: String,
    quantity: u32,
}

#[derive(Parser)]
#[command(author, version, about, long_about=None)]
struct Cli {
    #[arg(short, long)]
    broker_url: Option<String>,

    #[arg(short = 'p', long)]
    broker_port: Option<u16>,

    #[arg(short = 't', long)]
    topic_file: Option<String>,

    #[arg(short, long, action=clap::ArgAction::Count)]
    verbose: u8,

    #[arg(short = 'H', long, default_value_t = String::from("127.0.0.1:8080"))]
    http_server_url: String,
}

async fn subscribe_topics(client: &AsyncClient, cli: &Cli) -> Result<(), DMError> {
    let mut topics = vec!["test/topic".to_owned()];

    if let Some(f) = cli.topic_file.as_deref() {
        topics = tokio::fs::read_to_string(f)
            .await
            .map_err(|_e| Report::new(DMError::InvalidData))?
            .split('\n')
            .map(|a| a.to_owned())
            .collect();
    }

    for t in topics.iter() {
        let topic = t.trim();
        if topic.is_empty() {
            continue;
        }

        jinfo!("event=subscribe topic=[{}]", t);
        client
            .subscribe(topic, QoS::AtLeastOnce)
            .await
            .map_err(|_| Report::new(DMError::IOError))?;
    }

    Ok(())
}

async fn get_items() -> impl Responder {
    let items = vec![
        Item {
            id: 1,
            name: "item 1".to_owned(),
            quantity: 10,
        },
        Item {
            id: 2,
            name: "item 2".to_owned(),
            quantity: 5,
        },
    ];

    HttpResponse::Ok().json(items)
}

async fn create_item(item: web::Json<Item>) -> impl Responder {
    HttpResponse::Created().json(item.into_inner())
}

async fn http_work(mut rx: tokio::sync::broadcast::Receiver<String>) -> Result<(), DMError> {
    let cli = Cli::parse();
    eprintln!("{}", cli.http_server_url);

    tokio::select! {
        result = HttpServer::new(|| {
                        App::new()
                            .route("/items", web::get().to(get_items))
                            .route("/items", web::post().to(create_item))
                    })
                    .bind(&cli.http_server_url)
                    .map_err(|e| Report::new(DMError::IOError).attach_printable(e))?
                    .run() => {
                        result.map_err(|e| Report::new(DMError::IOError).attach_printable(e))?;

                    }

        _result = rx.recv() => {}
    }

    Ok(())
}

async fn do_test(client: &AsyncClient) {
    let internal = json::object! {
        "req_info" : {
            "req_id" : "0"
        },
        "led_enabled" : false,
        "log_settings" : [
            {
                "filter" : "main",
                "level" : 3,
                "destination" : 0,
                "storage_name" : "<storage_name>",
                "path" : "<file_path>"
            },
            {
                "filter" : "sensor",
                "level" : 2,
                "destination" : 1,
                "storage_name" : "<storage_name>",
                "path" : "<file_path>"
            }
        ],
        "temperature_update_interval" : 10,
        "res_info" : {
            "res_id": "0",
            "code": 0,
            "detail_msg": "ok"
        }
    };

    let mut system_settings = json::JsonValue::new_object();
    system_settings
        .insert(
            "configuration/$system/system_settings",
            internal.to_string(),
        )
        .unwrap();

    let payload = system_settings.to_string();

    jinfo!("Publish configuration");
    jdebug!("escaped json: {}", payload);

    client
        .publish("v1/devices/me/attributes", QoS::AtLeastOnce, false, payload)
        .await
        .unwrap();
}

async fn process_device_connect_req(
    client: &AsyncClient,
    topic: &str,
    payload: &str,
) -> Result<bool, DMError> {
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

        client
            .publish(
                &format!("v1/devices/{who}/attributes/response/{req_id}"),
                QoS::AtLeastOnce,
                false,
                payload,
            )
            .await
            .map_err(|_| Report::new(DMError::IOError))?;

        return Ok(true);
    }

    Ok(false)
}

async fn process_device_connect_rsp(
    client: &AsyncClient,
    topic: &str,
    payload: &str,
) -> Result<bool, DMError> {
    let re = Regex::new(r"^v1\/devices\/([^\/]+)\/attributes\/response\/(\d+)$")
        .map_err(|_| DMError::InvalidData)?;

    if let Some(caps) = re.captures(topic) {
        let who = &caps[1];
        let req_id: u32 = caps[2].parse().unwrap();

        jinfo!(
            func = "process_device_connect_rsp",
            topic = topic,
            payload = payload,
            who = who,
            req_id = req_id
        );

        let client = client.clone();
        tokio::task::spawn(async move {
            loop {
                do_test(&client).await;

                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        });

        return Ok(true);
    }

    Ok(false)
}

async fn process_agent_request(
    client: &AsyncClient,
    topic: &str,
    payload: &str,
) -> Result<bool, DMError> {
    let mut ret = process_device_connect_req(client, topic, payload).await?;
    if ret {
        return Ok(true);
    }

    ret = process_device_connect_rsp(client, topic, payload).await?;
    if ret {
        return Ok(true);
    }

    Ok(false)
}

async fn on_message(client: &AsyncClient, topic: &str, payload: &str) -> Result<(), DMError> {
    let v = json::parse(payload).unwrap();
    let pretty = json::stringify_pretty(v, 4);

    jinfo!(event = "publish", topic = topic);
    eprintln!("payload = {pretty}");

    loop {
        let mut ret = process_agent_request(client, topic, payload).await?;
        if ret {
            break;
        }

        return Err(Report::new(DMError::InvalidData));
    }

    Ok(())
}

async fn mqtt_work(mut rx: tokio::sync::broadcast::Receiver<String>) -> Result<(), DMError> {
    let cli = Cli::parse();
    let broker_url = cli.broker_url.as_deref().unwrap_or("localhost");
    let broker_port = cli.broker_port.unwrap_or(1883);
    let mut mqtt_options = MqttOptions::new("device-monitor", broker_url, broker_port);

    mqtt_options.set_keep_alive(tokio::time::Duration::from_secs(60));
    let (client, mut event_loop) = AsyncClient::new(mqtt_options, 10);
    subscribe_topics(&client, &cli).await?;

    'main: loop {
        tokio::select! {
            result = event_loop.poll() => {
                match result {
                    Ok(notification) => {
                        if let rumqttc::Event::Incoming(rumqttc::Incoming::Publish(p)) = notification {
                            let topic = &p.topic;
                            let payload = String::from_utf8(p.payload.to_vec()).unwrap();
                            if let Err(e) = on_message(&client, topic, &payload).await {
                                eprintln!("Err: on_message: {:?}", e);
                            }
                        }
                    }

                    Err(e) => {
                        eprintln!("Error: {e}");
                        break 'main;
                    }
                }
            }

            _ = rx.recv() => {
                    break 'main;
            }
        }
    }

    Ok(())
}

async fn async_main() -> Result<(), DMError> {
    let local = tokio::task::LocalSet::new();

    local
        .run_until(async {
            let (tx, rx1) = tokio::sync::broadcast::channel(100);
            let rx2 = tx.subscribe();

            let task1 = tokio::task::spawn_local(http_work(rx1));
            let task2 = tokio::task::spawn_local(mqtt_work(rx2));
            let _ = tokio::task::spawn_local(async move {
                let _ = tokio::signal::ctrl_c().await;
                let _ = tx.send(String::from("quit"));
            });

            task1
                .await
                .map_err(|e| Report::new(DMError::IOError).attach_printable(e))??;
            task2
                .await
                .map_err(|e| Report::new(DMError::IOError).attach_printable(e))??;

            Ok(())
        })
        .await
}

fn main() {
    let cli = Cli::parse();

    let level = match cli.verbose {
        1 => LevelFilter::DEBUG,
        2 => LevelFilter::TRACE,
        _ => LevelFilter::INFO,
    };

    JloggerBuilder::new()
        .max_level(level)
        .log_time(LogTimeFormat::TimeLocal)
        .build();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("tokio-runtime-device-monitor")
        .build()
        .unwrap();

    rt.block_on(async { async_main().await.unwrap() })
}
