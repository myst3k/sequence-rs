pub mod base;

use crate::ClientResult;
pub use base::BaseClient;
use serde::Deserialize;

pub(crate) fn convert_result<'a, T: Deserialize<'a>>(input: &'a str) -> ClientResult<T> {
    serde_json::from_str::<T>(input).map_err(Into::into)
}
