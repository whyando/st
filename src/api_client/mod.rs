pub mod api_models;

use crate::config::CONFIG;
use crate::models::*;
use core::panic;
use log::*;
use reqwest::{self, Method, StatusCode};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex, RwLock};
use tokio::time::Instant;

#[derive(Debug, Clone)]
pub struct ApiClient {
    base_url: String,
    client: reqwest::Client,
    agent_token: Arc<RwLock<Option<String>>>,
    next_request_ts: Arc<Mutex<Option<Instant>>>,
}

impl Default for ApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiClient {
    pub fn new() -> ApiClient {
        let user_agent = format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        let client = reqwest::ClientBuilder::new()
            .user_agent(user_agent)
            .timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .http1_only()
            .build()
            .unwrap();
        ApiClient {
            client,
            base_url: CONFIG.api_base_url.to_string(),
            agent_token: Arc::new(RwLock::new(None)),
            next_request_ts: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_agent_token(&self, token: &str) {
        let mut agent_token = self.agent_token.write().unwrap();
        if agent_token.is_some() {
            panic!("Cannot set agent token while agent token is already set");
        }
        *agent_token = Some(token.to_string());
    }

    pub async fn status(&self) -> Status {
        self.get("/").await
    }

    pub fn agent_token(&self) -> Option<String> {
        self.agent_token.read().unwrap().clone()
    }

    pub async fn register(&self, faction: &str, callsign: &str) -> String {
        let faction = match faction {
            "" => {
                let factions: Vec<Faction> = self.get_all_pages("/factions").await;
                let factions: Vec<Faction> =
                    factions.into_iter().filter(|f| f.is_recruiting).collect();
                use rand::prelude::SliceRandom as _;
                let faction = factions.choose(&mut rand::thread_rng()).unwrap();
                faction.symbol.clone()
            }
            _ => faction.to_string(),
        };
        assert!(
            self.agent_token().is_none(),
            "Cannot register while agent token is already set"
        );
        debug!(
            "Registering new agent {} with faction {}",
            callsign, faction
        );
        let mut body: Value = self
            .post(
                "/register",
                &json!({
                    "faction": faction,
                    "symbol": callsign,
                }),
            )
            .await;
        let _agent: Agent = serde_json::from_value(body["data"]["agent"].take()).unwrap();
        let _contract: Contract = serde_json::from_value(body["data"]["contract"].take()).unwrap();
        let _faction: Faction = serde_json::from_value(body["data"]["faction"].take()).unwrap();
        let _ship: Ship = serde_json::from_value(body["data"]["ship"].take()).unwrap();
        let token: String = body["data"]["token"].as_str().unwrap().to_string();

        token
    }

    pub async fn get_agent(&self) -> Agent {
        let response: Data<Agent> = self.get("/my/agent").await;
        response.data
    }

    pub async fn get_agent_public(&self, callsign: &str) -> Agent {
        let response: Data<Agent> = self.get(&format!("/agents/{}", callsign)).await;
        response.data
    }

    pub async fn get_ship(&self, id: &str) -> Ship {
        let response: Data<Ship> = self.get(&format!("/my/ships/{}", id)).await;
        response.data
    }

    pub async fn get_all_ships(&self) -> Vec<Ship> {
        self.get_all_pages("/my/ships").await
    }

    pub async fn get_system(&self, system_symbol: &SystemSymbol) -> api_models::System {
        let system: Data<api_models::System> =
            self.get(&format!("/systems/{}", system_symbol)).await;
        system.data
    }

    pub async fn get_system_waypoints(
        &self,
        system_symbol: &SystemSymbol,
    ) -> Vec<api_models::WaypointDetailed> {
        self.get_all_pages(&format!("/systems/{}/waypoints", system_symbol))
            .await
    }

    pub async fn get_market_remote(&self, symbol: &WaypointSymbol) -> MarketRemoteView {
        let market: Data<MarketRemoteView> = self
            .get(&format!(
                "/systems/{}/waypoints/{}/market",
                symbol.system(),
                symbol
            ))
            .await;
        market.data
    }

    pub async fn get_shipyard_remote(&self, symbol: &WaypointSymbol) -> ShipyardRemoteView {
        let shipyard: Data<ShipyardRemoteView> = self
            .get(&format!(
                "/systems/{}/waypoints/{}/shipyard",
                symbol.system(),
                symbol
            ))
            .await;
        shipyard.data
    }

    pub async fn get_construction(
        &self,
        symbol: &WaypointSymbol,
    ) -> WithTimestamp<Option<Construction>> {
        let path = format!(
            "/systems/{}/waypoints/{}/construction",
            symbol.system(),
            symbol
        );
        let (code, construction): (StatusCode, Result<Data<Construction>, String>) =
            self.request(Method::GET, &path, None::<&()>).await;
        let construction = match code {
            StatusCode::OK => Some(construction.unwrap().data),
            StatusCode::NOT_FOUND => None,
            _ => panic!("Request failed: {} {} {}", code.as_u16(), Method::GET, path),
        };
        WithTimestamp::<Option<Construction>> {
            timestamp: chrono::Utc::now(),
            data: construction,
        }
    }

    pub async fn get_jumpgate_conns(&self, symbol: &WaypointSymbol) -> Vec<WaypointSymbol> {
        let path = format!(
            "/systems/{}/waypoints/{}/jump-gate",
            symbol.system(),
            symbol
        );
        let mut response: Value = self.get(&path).await;
        let connections: Vec<WaypointSymbol> =
            serde_json::from_value(response["data"]["connections"].take()).unwrap();
        connections
        // let path = format!(
        //     "/systems/{}/waypoints/{}/jump-gate",
        //     symbol.system(),
        //     symbol
        // );
        // let (status, resp_body): (StatusCode, Result<Value, String>) =
        //     self.request(Method::GET, &path, None::<&()>).await;
        // let connections = match status {
        //     StatusCode::OK => {
        //         let mut response = resp_body.unwrap();
        //         let connections: Vec<WaypointSymbol> =
        //             serde_json::from_value(response["data"]["connections"].take()).unwrap();
        //         JumpGateConnections::Charted(connections)
        //     }
        //     StatusCode::BAD_REQUEST => {
        //         let response: Value = serde_json::from_str(&resp_body.unwrap_err()).unwrap();
        //         let code = response["error"]["code"].as_i64().unwrap();
        //         if code == 4001 {
        //             // 400 {"error":{"message":"Waypoint X1-XS84-X11D is not accessible. Either the waypoint is uncharted or the agent has no ships present at the location.","code":4001,"data":{"waypointSymbol":"X1-XS84-X11D"}}}
        //             JumpGateConnections::Uncharted
        //         } else {
        //             panic!(
        //                 "Request failed: {} {} {}",
        //                 status.as_u16(),
        //                 Method::GET,
        //                 path
        //             );
        //         }
        //     }
        //     _ => panic!(
        //         "Request failed: {} {} {}",
        //         status.as_u16(),
        //         Method::GET,
        //         path
        //     ),
        // };
        // JumpGateInfo {
        //     timestamp: chrono::Utc::now(),
        //     connections,
        // }
    }

    pub async fn get_all_pages<T>(&self, path: &str) -> Vec<T>
    where
        T: serde::de::DeserializeOwned,
    {
        #[allow(non_snake_case)]
        let PAGE_SIZE = 20;
        let mut page = 1;
        let mut vec = Vec::new();
        loop {
            let response: PaginatedList<T> = self
                .get(&format!("{}?page={}&limit={}", path, page, PAGE_SIZE))
                .await;
            vec.extend(response.data);
            if response.meta.page * PAGE_SIZE >= response.meta.total {
                break;
            }
            page += 1;
        }
        vec
    }
}

/// Private methods

impl ApiClient {
    pub async fn get<T>(&self, path: &str) -> T
    where
        T: serde::de::DeserializeOwned,
    {
        let (status, body_result) = self.request(Method::GET, path, None::<&()>).await;
        body_result.unwrap_or_else(|body| {
            panic!(
                "Request failed: {} {} {}\nbody: {}",
                status.as_u16(),
                Method::GET,
                path,
                body
            )
        })
    }

    pub async fn post<T, U>(&self, path: &str, json_body: &U) -> T
    where
        T: serde::de::DeserializeOwned,
        U: Serialize,
    {
        let (status, body_result) = self.request(Method::POST, path, Some(json_body)).await;
        body_result.unwrap_or_else(|body| {
            panic!(
                "Request failed: {} {} {}\nbody: {}",
                status.as_u16(),
                Method::POST,
                path,
                body
            )
        })
    }

    pub async fn patch<T, U>(&self, path: &str, json_body: &U) -> T
    where
        T: serde::de::DeserializeOwned,
        U: Serialize,
    {
        let (status, body_result) = self.request(Method::PATCH, path, Some(json_body)).await;
        body_result.unwrap_or_else(|body| {
            panic!(
                "Request failed: {} {} {}\nbody: {}",
                status.as_u16(),
                Method::PATCH,
                path,
                body
            )
        })
    }

    async fn wait_rate_limit(&self) {
        let now = Instant::now();
        let request_instant = {
            let mut next_request_ts = self.next_request_ts.lock().unwrap();
            let request_instant = match *next_request_ts {
                Some(ts) if ts > now => ts,
                _ => now,
            };
            *next_request_ts = Some(request_instant + std::time::Duration::from_millis(501));
            request_instant
        };
        let wait_duration = request_instant
            .checked_duration_since(now)
            .unwrap_or_default();
        if wait_duration >= std::time::Duration::from_secs(10) {
            warn!(
                "Rate limit queue exceeds 10 seconds: {:.3}s",
                wait_duration.as_secs_f64()
            );
        }
        tokio::time::sleep_until(request_instant).await;
    }

    pub async fn request<T, U>(
        &self,
        method: reqwest::Method,
        path: &str,
        json_body: Option<&U>,
    ) -> (StatusCode, Result<T, String>)
    where
        T: serde::de::DeserializeOwned,
        U: Serialize,
    {
        self.wait_rate_limit().await;
        let url = format!("{}{}", self.base_url, path);
        let mut request = self.client.request(method.clone(), &url);
        if let Some(body) = json_body {
            request = request.json(body);
        }
        if let Some(token) = self.agent_token() {
            request = request.header("Authorization", format!("Bearer {}", token));
        }
        let response = request.send().await.expect("Failed to send request");
        let status = response.status();
        debug!("{} {} {}", status.as_u16(), method, path);

        if status.is_success() {
            let content = response
                .json::<T>()
                .await
                .expect("Failed to parse successful response as json");
            (status, Ok(content))
        } else {
            let body = response
                .text()
                .await
                .expect("Failed to read response body from failed request");
            (status, Err(body))
        }
    }
}
