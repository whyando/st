use crate::{
    agent_controller::{AgentController, Event},
    data::DataClient,
};
use log::*;
use socketioxide::{extract::SocketRef, SocketIo};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, services::ServeDir};

pub struct WebApiServer {
    agent_controller: Arc<AgentController>,
    #[allow(dead_code)]
    db_client: Arc<DataClient>,
}

impl WebApiServer {
    pub fn new(agent_controller: &Arc<AgentController>, db_client: &Arc<DataClient>) -> Self {
        Self {
            agent_controller: agent_controller.clone(),
            db_client: db_client.clone(),
        }
    }

    pub async fn run(&self) {
        info!("Starting server");

        let (layer, io) = SocketIo::builder().build_layer();

        io.ns("/", |s: SocketRef| {
            // s.on("new message", |s: SocketRef, Data::<String>(msg)| {
            //     let username = s.extensions.get::<Username>().unwrap().clone();
            //     let msg = Res::Message {
            //         username,
            //         message: msg,
            //     };
            //     s.broadcast().emit("new message", msg).ok();
            // });
            info!("socket connected");

            s.on_disconnect(|_s: SocketRef| {
                info!("socket disconnected");
            });
        });

        let (tx, rx) = std::sync::mpsc::channel();
        self.agent_controller.add_event_listener(tx);

        tokio::spawn(async move {
            while let Ok(event) = rx.recv() {
                match event {
                    Event::ShipUpdate(ship) => {
                        io.emit("ship_upd", ship).ok();
                    }
                    Event::AgentUpdate(agent) => {
                        io.emit("agent_upd", agent).ok();
                    }
                }
            }
        });

        let app = axum::Router::new()
            .nest_service("/", ServeDir::new("dist"))
            .layer(
                ServiceBuilder::new()
                    .layer(CorsLayer::permissive()) // Enable CORS policy
                    .layer(layer),
            );

        let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
        axum::serve(listener, app).await.unwrap();
    }
}
