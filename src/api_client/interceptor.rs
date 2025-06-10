use reqwest::{Method, StatusCode};

/// Trait for intercepting API responses
pub trait ApiInterceptor: Send + Sync {
    /// Called after receiving an API response
    fn after_response(&self, method: &Method, path: &str, status: StatusCode, body: &str);
}
