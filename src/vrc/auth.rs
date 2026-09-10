use std::sync::Arc;

#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct Auth {
    pub basic_auth: Option<BasicAuth>,
    pub oauth_access_token: Option<Arc<str>>,
    pub bearer_access_token: Option<Arc<str>>,
    pub api_key: Option<ApiKey>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct BasicAuth {
    pub username: Arc<str>,
    pub password: Option<Arc<str>>,
}

impl Default for BasicAuth {
    fn default() -> Self {
        Self {
            username: Arc::from(""),
            password: None,
        }
    }
}

impl From<vrchatapi::apis::configuration::BasicAuth> for BasicAuth {
    fn from(value: vrchatapi::apis::configuration::BasicAuth) -> Self {
        Self {
            username: Arc::from(value.0),
            password: value.1.map(|p| Arc::from(p)),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ApiKey {
    pub prefix: Option<Arc<str>>,
    pub key: Arc<str>,
}

impl Default for ApiKey {
    fn default() -> Self {
        Self {
            prefix: None,
            key: Arc::from(""),
        }
    }
}
