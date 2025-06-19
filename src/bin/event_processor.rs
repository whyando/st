/// Simple event processor. Process events produced by the agent and insert a condensed form into scylla db.
use log::*;
use rdkafka::consumer::Consumer as _;
use rdkafka::consumer::StreamConsumer;
use rdkafka::message::Message as _;
use st::api_client::kafka_interceptor::ApiRequest;
use st::config::{KAFKA_CONFIG, KAFKA_TOPIC};
use st::scylla_client::ScyllaClient;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pretty_env_logger::init_timed();

    let worker = Worker::new().await;

    let consumer: StreamConsumer = KAFKA_CONFIG
        .clone()
        .set("group.id", "event-processor")
        .set("enable.auto.commit", "true")
        .set("auto.offset.reset", "earliest")
        .create()
        .expect("Failed to create Kafka consumer");

    consumer.subscribe(&[*KAFKA_TOPIC]).unwrap();

    info!("Subscribed to topic '{}'", *KAFKA_TOPIC);
    loop {
        let message = consumer.recv().await.unwrap();
        let topic = message.topic();
        let payload = message.payload().unwrap();
        if topic == *KAFKA_TOPIC {
            let api_request: ApiRequest = serde_json::from_slice(&payload).unwrap();
            worker.process_api_request(api_request).await;
        } else {
            panic!("Unknown topic: {}", topic);
        }
    }
}

struct Worker {
    scylla: ScyllaClient,
}

impl Worker {
    pub async fn new() -> Self {
        Self { scylla: ScyllaClient::new().await }
    }

    pub async fn process_api_request(&self, req: ApiRequest) {
        info!("Received api request: {} {} {} {}", req.request_id, req.status, req.method, req.path);
        let log_id = "test-log";
        let seq_num = self.scylla.get_next_seq_num(log_id).await;
        info!("Next seq num: {}", seq_num);
    }
}

