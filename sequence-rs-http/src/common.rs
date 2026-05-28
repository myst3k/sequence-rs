use serde_json::Value;
use std::collections::HashMap;
use std::fmt;

pub type Headers = HashMap<String, String>;

/// Query parameters as owned, ordered key/value pairs. Ordered (and
/// duplicate-friendly) so request URLs are deterministic and testable; owned so
/// callers don't have to keep the source strings alive for the request's
/// lifetime. Fed directly to `reqwest::RequestBuilder::query`.
pub type Query = Vec<(String, String)>;

#[allow(async_fn_in_trait)]
pub trait BaseHttpClient: Send + Default + Clone + fmt::Debug {
    type Error;

    async fn get(
        &self,
        url: &str,
        headers: Option<&Headers>,
        payload: &Query,
    ) -> Result<String, Self::Error>;

    async fn post(
        &self,
        url: &str,
        headers: Option<&Headers>,
        payload: &Value,
    ) -> Result<String, Self::Error>;
}
