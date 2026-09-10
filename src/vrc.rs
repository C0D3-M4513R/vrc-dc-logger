#![allow(unused)]
pub(crate) mod auth;
pub mod time;
pub mod websocket;

use serde::de::value::StringDeserializer;
use serde::de::{DeserializeOwned, SeqAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cell::{RefCell, RefMut};
use std::default::Default;
use std::fmt::{Debug, Display, Formatter};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, OnceLock, PoisonError, RwLockWriteGuard};
use std::time::Duration;
use futures::{FutureExt, StreamExt, TryFutureExt};
use http::header::InvalidHeaderValue;
use http::HeaderValue;
use once_cell::sync::Lazy;
use reqwest::cookie::CookieStore;
use tokio::sync::{Mutex, RwLock};
use tokio::sync::oneshot::Sender;
use tokio::time::timeout;
use tower::limit::RateLimit;
use tower::Service;
use vrchatapi::apis::configuration::Configuration;
use vrchatapi::apis::Error;
use vrchatapi::models::{CurrentUser, Success, TwoFactorAuthCode, TwoFactorEmailCode};

const DOMAIN: &str = "api.vrchat.cloud";
const BASE_PATH: &str = "https://api.vrchat.cloud/api/1";
pub const VRC_USER_AGENT:&str = "vrc-logger/private-indev c0d3m4513r@c0d3m4513r.com";

#[derive(Debug, Clone)]
//todo: I do not like the mutexes inside arc here at all.
///invariant: if logged_in is Some, then we are logged in.
pub struct Vrc {
    //serde skip. Loaded in deserialize, and saved to cookie_file in serialize
    cookie_store: Arc<reqwest_cookie_store::CookieStoreRwLock>,
    //serde skip. Loaded in Login method, reset in logout method.
    logged_in: Arc<RwLock<Option<Arc<vrchatapi::models::CurrentUser>>>>,
    websocket_terminator: Arc<RwLock<Option<tokio::sync::oneshot::Sender<()>>>>,
    auth: Arc<RwLock<auth::Auth>>,
    //serde skip
    client: reqwest::Client,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct VrcSerializable<'a> {
    pub cookie_file: Box<[cookie_store::Cookie<'a>]>,
    pub login: Option<Arc<vrchatapi::models::CurrentUser>>,
    pub auth: auth::Auth,
}

impl Default for Vrc {
    fn default() -> Self {
        let cookie_store = Arc::new(reqwest_cookie_store::CookieStoreRwLock::default());
        let client = reqwest::Client::builder()
            .cookie_provider(cookie_store.clone())
            .build()
            .unwrap();

        Self {
            cookie_store,
            logged_in: Arc::new(RwLock::new(None)),
            auth: Arc::new(RwLock::new(auth::Auth::default())),
            websocket_terminator: Arc::new(RwLock::default()),
            client,
        }
    }
}

impl Vrc {
    pub fn new(
        cookies: impl Iterator<Item = cookie_store::Cookie<'static>>,
    ) -> Result<Self, crate::Error> {
        let cookie_store = match reqwest_cookie_store::CookieStore::from_cookies(cookies.map(|value|Ok::<_, core::convert::Infallible>(value)), false){
            Ok(v) => v,
            Err(v) => match v {}
        };
        let cookie_store = Arc::new(reqwest_cookie_store::CookieStoreRwLock::new(cookie_store));
        let client = reqwest::Client::builder()
            .cookie_provider(cookie_store.clone())
            .build()?;

        Ok(Self {
                cookie_store,
                logged_in: Default::default(),
                auth: Default::default(),
                websocket_terminator: Arc::new(RwLock::default()),
                client,
            }
        )
    }
    pub async fn get_serializable(&self) -> VrcSerializable<'_> {
        let cookies = Box::from_iter(self.cookie_store.read().unwrap_or_else(|e| {
            log::error!("Failed to read cookie store, because of a PoisonError: {}. Trying to read anyways", e);
            e.into_inner()
        }).iter_unexpired().cloned());
        VrcSerializable {
            cookie_file: cookies,
            login: self.logged_in.read().await.clone(),
            auth: self.auth.read().await.clone(),
        }
    }
    ///This function will load all persistent data into a new Vrc struct.
    ///This function will break the invariant of the Vrc struct, because the user might not be logged in anymore.
    ///This invariant needs to be re-established by calling [Self::check_auth()].
    pub fn from_seralizable(
        serializable: VrcSerializable<'static>,
    ) -> Result<Self, crate::Error> {
        let mut vrc = Vrc::new(serializable.cookie_file.into_iter())?;
        vrc.auth = Arc::new(RwLock::new(serializable.auth));
        vrc.logged_in = Arc::new(RwLock::new(serializable.login));
        Ok(vrc)
    }

    pub async fn get_logged_in_user(&self) -> Option<Arc<vrchatapi::models::CurrentUser>> {
        self.logged_in.read().await.clone()
    }

    pub fn get_client(&self) -> reqwest::Client {
        self.client.clone()
    }

    pub async fn get_configuration(&self) -> vrchatapi::apis::configuration::Configuration {
        let mut config = vrchatapi::apis::configuration::Configuration {
            base_path: BASE_PATH.to_owned(),
            user_agent: Some(VRC_USER_AGENT.to_owned()),
            client: self.client.clone().into(),
            basic_auth: None,
            oauth_access_token: None,
            bearer_access_token: None,
            api_key: None,
        };
        let auth = self.auth.read().await;
        if let Some(auth) = &auth.basic_auth {
            config.basic_auth = Some((
                auth.username.to_string(),
                auth.password.as_ref().map(|p| p.to_string()),
            ));
        } else {
            config.basic_auth = None;
        }
        if let Some(token) = &auth.oauth_access_token {
            config.oauth_access_token = Some(token.to_string());
        }
        if let Some(token) = &auth.bearer_access_token {
            config.bearer_access_token = Some(token.to_string());
        }
        if let Some(key) = &auth.api_key {
            config.api_key = Some(vrchatapi::apis::configuration::ApiKey {
                prefix: key.prefix.as_ref().map(|p| p.to_string()),
                key: key.key.to_string(),
            })
        }
        config
    }

    pub async fn get_auth(&self) -> impl Deref<Target = auth::Auth> + '_ {
        self.auth.read().await
    }

    pub async fn get_auth_mut(&self) -> impl DerefMut + Deref<Target = auth::Auth> + '_ {
        self.auth.write().await
    }

    ///Sets the basic auth for the VRC struct.
    ///If the API is already logged in, this will return an error.
    ///Use logout() to log out of the API before calling this.
    pub async fn set_auth(
        &self,
        auth: vrchatapi::apis::configuration::BasicAuth,
    ) -> Result<(), ()> {
        let logged_in = self.logged_in.read().await;
        if logged_in.is_some() {
            return Err(());
        }
        self.auth.write().await.basic_auth = Some(auth.into());
        Ok(())
    }

    pub async fn test_ws<T: websocket::connection::WSHandler + Send + 'static>(&self, storage: T) {
        let ws = self.websocket_terminator.read().await;
        if let Some(v) = ws.as_ref() {
            if v.is_closed(){
                log::error!("Websocket was closed. Reopening");
                drop(ws);
                self.websocket_terminator.write().await.take();
                self.start_ws(storage).await;
            }
        }
    }

    async fn start_ws<T: websocket::connection::WSHandler + Send + 'static>(&self, storage: T) {
        if let Some(terminator) = self.websocket_terminator.write().await.take(){
            terminator.send(());
        }
        const EMPTY_HEADER_VALUE:http::HeaderValue = http::HeaderValue::from_static("");
        let cookies= {
            match self.cookie_store.read()
                .unwrap_or_else(|poison| poison.into_inner())
                .get(DOMAIN, "/", "auth")
            {
                Some(cookie) => {
                    match url::Url::parse_with_params(format!("https://{DOMAIN}/").as_str(), &[("authToken", cookie.value())]) {
                        Ok(url) => {
                            Some((cookie.value().to_string(), self.cookie_store.cookies(&url)))
                        },
                        Err(err) => {
                            log::error!("Cannot parse WS Url to get cookies, but was able to get auth cookie. The websocket conection might fail!");
                            Some((cookie.value().to_string(), http::header::HeaderValue::from_str(cookie.value()).ok()))
                        }
                    }
                }
                None => {
                    None
                }
            }
        };
        match cookies {
            Some((auth_token, cookies)) => {
                let websocket_terminator = self.websocket_terminator.clone();
                let cookies = cookies.unwrap_or(EMPTY_HEADER_VALUE);
                #[cfg(feature = "debug")]
                {
                    log::info!("Trying to start Websocket with auth token: {auth_token} and cookies: {cookies:#?}");
                }
                match websocket::connection::WebSocket::new(auth_token.as_str(), cookies, storage).await{
                    Ok(terminator_sender) => {
                        *websocket_terminator.write().await = Some(terminator_sender);
                        log::info!("Websocket Started");
                    }
                    Err(err) => {
                        log::error!("Error starting Websocket: {err}");
                    }
                }
            },
            None => {
                log::error!("Not starting Websocket, because auth token is not present at the expected domain, path and name (despite being logged in with the api)");
            }
        }
    }
    ///This will check if the auth token is still valid.
    ///This function may be used to re-establish the invariant of the Vrc struct.
    pub async fn check_auth<T: websocket::connection::WSHandler + Send + 'static>(&self, storage: T) -> Result<vrchatapi::models::RegisterUserAccount200Response, vrchatapi::apis::Error<vrchatapi::apis::authentication_api::GetCurrentUserError>> {
        let mut config = self.get_configuration().await;
        config.basic_auth = None;
        match vrchatapi::apis::authentication_api::get_current_user(&config).await {
            Ok(vrchatapi::models::RegisterUserAccount200Response::CurrentUser(user)) => {
                *self.logged_in.write().await = Some(Arc::new(user.clone()));
                self.start_ws(storage).await;
                Ok(vrchatapi::models::RegisterUserAccount200Response::CurrentUser(user))
            },
            Ok(v) => Ok(v),
            Err(error) => {
                log::error!("Failed to verify auth token. Assuming invalid and Purging Login Cache: {}", error);
                *self.logged_in.write().await = None;
                Err(error)
            }
        }
    }

    pub async fn login<T: websocket::connection::WSHandler + Send + 'static>(
        &self,
        exclude_basic_auth: bool,
        storage: T,
    ) -> Result<vrchatapi::models::RegisterUserAccount200Response, LoginError<()>> {
        if self.logged_in.read().await.is_some() {
            return Err(LoginError::AlreadyLoggedIn);
        }
        let mut config = self.get_configuration().await;
        if exclude_basic_auth {
            config.basic_auth = None;
        }
        match vrchatapi::apis::authentication_api::get_current_user(&config).await {
            Ok(vrchatapi::models::RegisterUserAccount200Response::RequiresTwoFactorAuth(two_fa)) => {
                Ok(vrchatapi::models::RegisterUserAccount200Response::RequiresTwoFactorAuth(two_fa))
            }
            Ok(vrchatapi::models::RegisterUserAccount200Response::CurrentUser(login)) => {
                *self.logged_in.write().await = Some(Arc::new(login.clone()));
                self.start_ws(storage).await;
                Ok(vrchatapi::models::RegisterUserAccount200Response::CurrentUser(login))
            }
            Err(e) => {
                log::error!("Failed to log in: {}", e);
                Err(LoginError::from(e))
            }
        }
    }

    pub async fn verify_2fa<T: websocket::connection::WSHandler + Send + 'static>(
        &self,
        two_fa: String,
        method: TwoFaMethod,
        storage: T,
    ) -> Result<bool, LoginError<()>> {
        let configuration = self.get_configuration().await;
        if match method {
            TwoFaMethod::Email => {
                vrchatapi::apis::authentication_api::verify2_fa_email_code(
                    &configuration,
                    TwoFactorEmailCode::new(two_fa.into()),
                )
                .await?
                .verified
            }
            TwoFaMethod::OTP => {
                vrchatapi::apis::authentication_api::verify2_fa(
                    &configuration,
                    TwoFactorAuthCode::new(two_fa.into()),
                )
                .await?
                .verified
            }
            TwoFaMethod::TOTP => {
                vrchatapi::apis::authentication_api::verify_recovery_code(
                    &configuration,
                    TwoFactorAuthCode::new(two_fa.into()),
                )
                .await?
                .verified
            }
        } {
            self.start_ws(storage).await;
            Ok(true)
        }else {
            Ok(false)
        }
    }

    pub async fn logout(&self) -> Result<(), LoginError<()>> {
        if self.logged_in.read().await.is_none() {
            return Err(LoginError::NotLoggedIn);
        }
        match vrchatapi::apis::authentication_api::logout(&self.get_configuration().await).await {
            Ok(_) => {
                *self.logged_in.write().await = None;
                self.auth.write().await.basic_auth = None;
                Ok(())
            }
            Err(e) => Err(LoginError::from(e)),
        }
    }
}

#[derive(Debug)]
pub enum LoginError<T: std::any::Any + Sized> {
    AlreadyLoggedIn,
    NotLoggedIn,
    Needs2fa,
    GetCurrentUserError(
        vrchatapi::apis::ResponseContent<vrchatapi::apis::authentication_api::GetCurrentUserError>,
    ),
    Verify2FaError(
        vrchatapi::apis::ResponseContent<vrchatapi::apis::authentication_api::Verify2FaError>,
    ),
    VerifyRecoveryCodeError(
        vrchatapi::apis::ResponseContent<
            vrchatapi::apis::authentication_api::VerifyRecoveryCodeError,
        >,
    ),
    Verify2FaEmailCodeError(
        vrchatapi::apis::ResponseContent<
            vrchatapi::apis::authentication_api::Verify2FaEmailCodeError,
        >,
    ),
    LogoutError(vrchatapi::apis::ResponseContent<vrchatapi::apis::authentication_api::LogoutError>),
    VrChat(crate::VRCError<T>),
}
crate::impl_from!(LoginError<T>[T:std::any::Any], crate::VRCError<T>, VrChat);
// crate::impl_from!(LoginError<T>[T:std::any::Any], delegate, e:vrchatapi::apis::Error<T>,LoginError::VrChat(crate::VRCError::Error(e)));
crate::impl_from!(LoginError<T>[T:std::any::Any], map, vrchatapi::apis::Error<vrchatapi::apis::authentication_api::GetCurrentUserError>,
    vrchatapi::apis::Error::ResponseError(e) => LoginError::GetCurrentUserError(e),
    vrchatapi::apis::Error::Reqwest(e) => LoginError::VrChat(crate::VRCError::Error(vrchatapi::apis::Error::Reqwest(e))),
    vrchatapi::apis::Error::ReqwestMiddleware(e) => LoginError::VrChat(crate::VRCError::Error(vrchatapi::apis::Error::ReqwestMiddleware(e))),
    vrchatapi::apis::Error::Io(e) => LoginError::VrChat(crate::VRCError::Error(vrchatapi::apis::Error::Io(e))),
    vrchatapi::apis::Error::Serde(e) => LoginError::VrChat(crate::VRCError::Error(vrchatapi::apis::Error::Serde(e))),
);
crate::impl_from!(LoginError<T>[T:std::any::Any], map, vrchatapi::apis::Error<vrchatapi::apis::authentication_api::Verify2FaError>,
    vrchatapi::apis::Error::ResponseError(e) => LoginError::Verify2FaError(e),
    vrchatapi::apis::Error::Reqwest(e) => LoginError::VrChat(crate::VRCError::Error(vrchatapi::apis::Error::Reqwest(e))),
    vrchatapi::apis::Error::ReqwestMiddleware(e) => LoginError::VrChat(crate::VRCError::Error(vrchatapi::apis::Error::ReqwestMiddleware(e))),
    vrchatapi::apis::Error::Io(e) => LoginError::VrChat(crate::VRCError::Error(vrchatapi::apis::Error::Io(e))),
    vrchatapi::apis::Error::Serde(e) => LoginError::VrChat(crate::VRCError::Error(vrchatapi::apis::Error::Serde(e))),
);
crate::impl_from!(LoginError<T>[T:std::any::Any], map, vrchatapi::apis::Error<vrchatapi::apis::authentication_api::VerifyRecoveryCodeError>,
    vrchatapi::apis::Error::ResponseError(e) => LoginError::VerifyRecoveryCodeError(e),
    vrchatapi::apis::Error::Reqwest(e) => LoginError::VrChat(crate::VRCError::Error(vrchatapi::apis::Error::Reqwest(e))),
    vrchatapi::apis::Error::ReqwestMiddleware(e) => LoginError::VrChat(crate::VRCError::Error(vrchatapi::apis::Error::ReqwestMiddleware(e))),
    vrchatapi::apis::Error::Io(e) => LoginError::VrChat(crate::VRCError::Error(vrchatapi::apis::Error::Io(e))),
    vrchatapi::apis::Error::Serde(e) => LoginError::VrChat(crate::VRCError::Error(vrchatapi::apis::Error::Serde(e))),
);
crate::impl_from!(LoginError<T>[T:std::any::Any], map, vrchatapi::apis::Error<vrchatapi::apis::authentication_api::Verify2FaEmailCodeError>,
    vrchatapi::apis::Error::ResponseError(e) => LoginError::Verify2FaEmailCodeError(e),
    vrchatapi::apis::Error::Reqwest(e) => LoginError::VrChat(crate::VRCError::Error(vrchatapi::apis::Error::Reqwest(e))),
    vrchatapi::apis::Error::ReqwestMiddleware(e) => LoginError::VrChat(crate::VRCError::Error(vrchatapi::apis::Error::ReqwestMiddleware(e))),
    vrchatapi::apis::Error::Io(e) => LoginError::VrChat(crate::VRCError::Error(vrchatapi::apis::Error::Io(e))),
    vrchatapi::apis::Error::Serde(e) => LoginError::VrChat(crate::VRCError::Error(vrchatapi::apis::Error::Serde(e))),
);
crate::impl_from!(LoginError<T>[T:std::any::Any], map, vrchatapi::apis::Error<vrchatapi::apis::authentication_api::LogoutError>,
    vrchatapi::apis::Error::ResponseError(e) => LoginError::LogoutError(e),
    vrchatapi::apis::Error::Reqwest(e) => LoginError::VrChat(crate::VRCError::Error(vrchatapi::apis::Error::Reqwest(e))),
    vrchatapi::apis::Error::ReqwestMiddleware(e) => LoginError::VrChat(crate::VRCError::Error(vrchatapi::apis::Error::ReqwestMiddleware(e))),
    vrchatapi::apis::Error::Io(e) => LoginError::VrChat(crate::VRCError::Error(vrchatapi::apis::Error::Io(e))),
    vrchatapi::apis::Error::Serde(e) => LoginError::VrChat(crate::VRCError::Error(vrchatapi::apis::Error::Serde(e))),
);
impl<T: std::any::Any> Display for LoginError<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LoginError::AlreadyLoggedIn => write!(f, "LoginError::AlreadyLoggedIn"),
            LoginError::NotLoggedIn => write!(f, "LoginError::NotLoggedIn"),
            LoginError::Needs2fa => write!(f, "LoginError::Needs2fa"),
            LoginError::GetCurrentUserError(e) => write!(f, "LoginError::GetCurrentUserError({e:?})"),
            LoginError::Verify2FaError(e) => write!(f, "LoginError::Verify2FaError({e:?}))"),
            LoginError::VerifyRecoveryCodeError(e) => {
                write!(f, "LoginError::VerifyRecoveryCodeError({e:?}))")
            }
            LoginError::Verify2FaEmailCodeError(e) => {
                write!(f, "LoginError::Verify2FaEmailCodeError({e:?}))")
            }
            LoginError::LogoutError(e) => write!(f, "LoginError::LogoutError({e:?})"),
            LoginError::VrChat(e) => write!(f, "LoginError::VrChat({e})"),
        }
    }
}
impl<T: std::any::Any + Debug> std::error::Error for LoginError<T> {
    fn cause(&self) -> Option<&dyn std::error::Error> {
        match self {
            LoginError::AlreadyLoggedIn => None,
            LoginError::NotLoggedIn => None,
            LoginError::Needs2fa => None,
            LoginError::GetCurrentUserError(_) => None,
            LoginError::Verify2FaError(_) => None,
            LoginError::VerifyRecoveryCodeError(_) => None,
            LoginError::Verify2FaEmailCodeError(_) => None,
            LoginError::LogoutError(_) => None,
            LoginError::VrChat(e) => Some(e),
        }
    }
}

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    serde::Deserialize,
    serde::Serialize,
    PartialEq,
    Eq,
    Ord,
    PartialOrd,
)]
pub enum TwoFaMethod {
    #[default]
    Email,
    OTP,
    TOTP,
}

impl Display for TwoFaMethod {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TwoFaMethod::Email => write!(f, "Email"),
            TwoFaMethod::OTP => write!(f, "OTP"),
            TwoFaMethod::TOTP => write!(f, "Recovery Code"),
        }
    }
}
