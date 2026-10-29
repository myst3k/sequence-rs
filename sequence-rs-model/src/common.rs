use serde::{Deserialize, Serialize};

/// Money amounts in the Sequence API are integer cents.
pub type Cents = i64;

/// Pagination metadata. The API exposes no `total`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    pub page: u32,
    pub page_size: u32,
    /// Whether another page exists. Older responses omit it, so it defaults to
    /// `false`; the `*_stream` helpers then fall back to "a full page continues".
    #[serde(default)]
    pub has_next_page: bool,
}

/// The `data` of a paginated list response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paginated<T> {
    pub items: Vec<T>,
    pub pagination: Pagination,
}

/// Envelope wrapping every response; `request_id` is worth logging on failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse<T> {
    pub data: T,
    pub request_id: String,
}
