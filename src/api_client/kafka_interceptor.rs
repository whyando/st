use crate::api_client::interceptor::ApiInterceptor;
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use log::*;
use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use reqwest::{Method, StatusCode};
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

lazy_static! {
    pub static ref KAFKA_TOPIC: &'static str = "api-responses";
    pub static ref KAFKA_CONFIG: ClientConfig = {
        let kafka_url = std::env::var("KAFKA_URL").expect("KAFKA_URL must be set");
        let kafka_username = std::env::var("KAFKA_USERNAME").expect("KAFKA_USERNAME must be set");
        let kafka_password = std::env::var("KAFKA_PASSWORD").expect("KAFKA_PASSWORD must be set");
        let mut config = ClientConfig::new();
        config
            .set("bootstrap.servers", kafka_url)
            .set("security.protocol", "SASL_PLAINTEXT")
            .set("sasl.mechanism", "PLAIN")
            // jpa note: use PLAIN for now, seems like SCRAM is broken atm in the rdkafka crate (perhaps since kafka 4.0.0)
            // .set("sasl.mechanism", "SCRAM-SHA-256")
            .set("sasl.username", kafka_username)
            .set("sasl.password", kafka_password);
        config
    };
}

pub async fn init_kafka_topic() {
    let admin_client: AdminClient<_> = KAFKA_CONFIG
        .create()
        .expect("Failed to create Kafka admin client");

    let new_topic = NewTopic::new(
        &KAFKA_TOPIC,
        1,                          // num_partitions
        TopicReplication::Fixed(1), // replication_factor
    )
    .set("cleanup.policy", "delete")
    .set("retention.bytes", "1000000000") // 1GB
    .set("retention.ms", "86400000"); // 24 hours

    info!("Creating topic {}", *KAFKA_TOPIC);
    let create_topic_result = admin_client
        .create_topics(&[new_topic], &AdminOptions::new())
        .await;
    match create_topic_result {
        Ok(_) => info!("Successfully configured topic {}", *KAFKA_TOPIC),
        Err(e) => {
            panic!("Failed to configure topic {}: {}", *KAFKA_TOPIC, e);
        }
    }
}

#[derive(Clone, Serialize)]
struct ApiRequest {
    timestamp: DateTime<Utc>,
    request_id: u64,
    method: String,
    path: String,
    status: u16,
    request_body: String,
    response_body: String,
}

enum KafkaMessage {
    ApiRequest(ApiRequest),
}

#[derive(Debug)]
pub struct KafkaInterceptor {
    sender: mpsc::Sender<KafkaMessage>,
    hdl: Arc<tokio::sync::Mutex<JoinHandle<()>>>,
}

impl KafkaInterceptor {
    pub async fn new() -> Self {
        init_kafka_topic().await;
        let (sender, mut receiver) = mpsc::channel::<KafkaMessage>(10);

        let producer: FutureProducer = KAFKA_CONFIG
            .create()
            .expect("Failed to create Kafka producer");

        // Spawn background task for Kafka publishing
        let hdl = tokio::spawn(async move {
            while let Some(message) = receiver.recv().await {
                match message {
                    KafkaMessage::ApiRequest(data) => {
                        let producer = producer.clone();
                        if let Err(e) = producer
                            .send(
                                FutureRecord::to(&KAFKA_TOPIC)
                                    .payload(&serde_json::to_string(&data).unwrap())
                                    .key("response"),
                                Duration::from_secs(5),
                            )
                            .await
                        {
                            error!("Failed to send kafka message: {:?}", e);
                        }
                    }
                }
            }
        });

        Self {
            sender,
            hdl: Arc::new(tokio::sync::Mutex::new(hdl)),
        }
    }

    pub async fn join(&self) {
        let mut hdl = self.hdl.lock().await;
        (&mut *hdl).await.unwrap();
    }
}

impl ApiInterceptor for KafkaInterceptor {
    fn after_response(
        &self,
        req_id: u64,
        method: &Method,
        path: &str,
        status: StatusCode,
        request_body: &str,
        response_body: &str,
    ) {
        let message = KafkaMessage::ApiRequest(ApiRequest {
            timestamp: Utc::now(),
            request_id: req_id,
            method: method.to_string(),
            path: path.to_string(),
            status: status.as_u16(),
            request_body: request_body.to_string(),
            response_body: response_body.to_string(),
        });

        // Non-blocking send - if channel is full or disconnected, drop the message
        if let Err(e) = self.sender.try_send(message) {
            warn!("Failed to send to channel: {:?}", e);
        }
    }
}
