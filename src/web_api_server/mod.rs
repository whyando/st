use crate::{
    agent_controller::{AgentController, Event},
    data::DataClient,
    models::{Agent, Ship, Waypoint},
    universe::Universe,
};
use axum::{debug_handler, http::StatusCode};
use axum::{extract::State, routing::get};
use log::*;
use socketioxide::{
    extract::{Data, SocketRef},
    SocketIo, TransportType,
};
use std::{sync::Arc, time::Duration};
use tower_http::cors::CorsLayer;

pub struct WebApiServer {
    agent_controller: AgentController,
    db_client: DataClient,
    universe: Universe,
}

struct AppState {
    agent_controller: AgentController,
    #[allow(dead_code)]
    db_client: DataClient,
    universe: Universe,
}

#[debug_handler]
async fn agent_handler(State(state): State<Arc<AppState>>) -> axum::Json<Agent> {
    let agent = state.agent_controller.agent();
    axum::Json(agent)
}

#[debug_handler]
async fn ships_handler(State(state): State<Arc<AppState>>) -> axum::Json<Vec<Ship>> {
    let ships = state.agent_controller.ships();
    axum::Json(ships)
}

#[debug_handler]
async fn waypoints_handler(
    State(state): State<Arc<AppState>>,
) -> Result<axum::Json<Vec<Waypoint>>, StatusCode> {
    let system_symbol = state.agent_controller.starting_system(); // todo: make this a parameter
    let waypoints_opt = state
        .universe
        .get_system_waypoints_no_fetch(&system_symbol)
        .await;
    match waypoints_opt {
        Some(waypoints) => Ok(axum::Json(waypoints)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

#[debug_handler]
async fn handler() -> () {}

async fn background_task(io: SocketIo, mut rx: tokio::sync::mpsc::Receiver<Event>) {
    while let Some(event) = rx.recv().await {
        match event {
            Event::ShipUpdate(ship) => {
                io.of("/").unwrap().emit("ship_upd", ship).unwrap();
            }
            Event::AgentUpdate(agent) => {
                io.of("/").unwrap().emit("agent_upd", agent).unwrap();
            }
        }
    }
}

impl WebApiServer {
    pub fn new(
        agent_controller: &AgentController,
        db_client: &DataClient,
        universe: &Universe,
    ) -> Self {
        Self {
            agent_controller: agent_controller.clone(),
            db_client: db_client.clone(),
            universe: universe.clone(),
        }
    }

    pub async fn run(&self) {
        info!("Starting server");

        let (socketio_layer, io) = SocketIo::builder()
            .req_path("/")
            .transports([TransportType::Websocket])
            .ping_interval(Duration::from_secs(1))
            .ping_timeout(Duration::from_secs(1))
            .build_layer();

        io.ns("/", |s: SocketRef| {
            info!("socket connected");

            s.emit("hello", "world").ok();
            s.on("ping", |s: SocketRef, Data::<i64>(data)| {
                info!("ping received {}", data);
                s.emit("pong", data).unwrap();
            });

            s.on_disconnect(|_s: SocketRef| {
                info!("socket disconnected");
            });
        });

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        let hdl = {
            let io = io.clone();
            tokio::spawn(background_task(io, rx))
        };
        self.agent_controller.add_event_listener(tx);

        let shared_state = Arc::new(AppState {
            agent_controller: self.agent_controller.clone(),
            db_client: self.db_client.clone(),
            universe: self.universe.clone(),
        });

        let app = axum::Router::new()
            .route("/api/agent", get(agent_handler))
            .route("/api/ships", get(ships_handler))
            .route("/api/waypoints", get(waypoints_handler))
            .route("/api/events", get(handler).layer(socketio_layer))
            .with_state(shared_state)
            .layer(CorsLayer::permissive());

        let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
        let server = async {
            info!("Listening on {}", listener.local_addr().unwrap());
            axum::serve(listener, app).await.unwrap();
        };

        let _ = tokio::join!(hdl, server);
    }
}
