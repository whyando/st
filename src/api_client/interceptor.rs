use reqwest::{Method, StatusCode};

/// Trait for intercepting API responses
pub trait ApiInterceptor: Send + Sync {
    /// Called after receiving an API response
    fn after_response(
        &self,
        req_id: u64,
        method: &Method,
        path: &str,
        status: StatusCode,
        request_body: &str,
        response_body: &str,
    );
}
