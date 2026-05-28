use crate::clients::BaseClient;
use crate::{Config, Credentials};
use sequence_rs_http::HttpClient;

/// Top-level client for the Sequence Platform API.
#[derive(Clone, Debug, Default)]
pub struct Sequence {
    pub config: Config,
    pub creds: Credentials,
    pub(crate) http: HttpClient,
}

impl Sequence {
    /// Client with default config (production URL, default retry + rate limit).
    pub fn new(creds: Credentials) -> Self {
        Self::with_config(creds, Config::default())
    }

    /// Client with a custom `Config` (base URL, retry, rate limit, …).
    pub fn with_config(creds: Credentials, config: Config) -> Self {
        let http = HttpClient::with_config(config.retry, config.rate_limit);
        Self {
            creds,
            config,
            http,
        }
    }
}

#[allow(async_fn_in_trait)]
impl BaseClient for Sequence {
    fn get_config(&self) -> &Config {
        &self.config
    }

    fn get_http(&self) -> &HttpClient {
        &self.http
    }

    fn get_creds(&self) -> &Credentials {
        &self.creds
    }
}
