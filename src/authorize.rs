use std::net::{IpAddr, Ipv4Addr};
use std::ops::{Deref, DerefMut};
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use reqwest::{Client, Request, RequestBuilder};
use rocket::response::Redirect;
use rocket::{get, Shutdown, State};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::authorization_endpoint;

const AUTHORIZATION_SCOPES: &str = "user-top-read playlist-read-private playlist-modify-private";

#[derive(Debug, Deserialize, Serialize)]
pub struct SpautofyConfigFile {
    address: Option<IpAddr>,
    port: Option<u16>,
    client_id: String,
    client_secret: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SpautofyConfig {
    pub address: IpAddr,
    pub port: u16,
    client_id: String,
    client_secret: String,
    pub user_auth_code: Option<String>,
    random_state: String,
}

impl From<&SpautofyConfig> for SpautofyConfigFile {
    fn from(config: &SpautofyConfig) -> Self {
        SpautofyConfigFile {
            address: Some(config.address),
            port: Some(config.port),
            client_id: config.client_id.clone(),
            client_secret: config.client_secret.clone(),
        }
    }
}

impl From<SpautofyConfigFile> for SpautofyConfig {
    fn from(file_config: SpautofyConfigFile) -> Self {
        SpautofyConfig {
            address: file_config
                .address
                .unwrap_or(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
            port: file_config.port.unwrap_or(3000),
            client_id: file_config.client_id,
            client_secret: file_config.client_secret,
            user_auth_code: None,
            random_state: random_state(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Access {
    access_token: String,
    scope: String,
    expires_in: i32,
    refresh_token: String,
    #[serde(skip, default = "Instant::now")]
    received_at: Instant,
}

impl Access {
    fn is_expired(&self) -> bool {
        self.received_at.elapsed().as_secs() > self.expires_in as u64
    }
    pub fn authorize(&self, request_builder: RequestBuilder) -> RequestBuilder {
        request_builder.bearer_auth(self.access_token.as_str())
    }
}

#[derive(Debug, Error)]
pub enum AuthorizeError {
    #[error("Have not received user authorization yet.")]
    NoUserAuthCode,
    #[error("User code has expired.")]
    ExpiredUserCode,
    #[error("Request error: {0}")]
    RequestError(reqwest::Error),
    #[error("Unknown error.")]
    Unknown,
}

impl From<reqwest::Error> for AuthorizeError {
    fn from(err: reqwest::Error) -> Self {
        AuthorizeError::RequestError(err)
    }
}

fn random_state() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect()
}

impl SpautofyConfig {
    pub fn needs_auth(&self) -> bool {
        self.user_auth_code.is_none()
    }

    fn redirect_url(&self) -> String {
        format!("http://{}:{}/callback", self.address, self.port)
    }

    fn auth_request(&self) -> Request {
        Client::new()
            .get(authorization_endpoint!("/authorize"))
            .query(&[
                ("client_id", self.client_id.as_str()),
                ("response_type", "code"),
                ("redirect_uri", self.redirect_url().as_str()),
                ("scope", AUTHORIZATION_SCOPES),
                ("show_dialog", "true"),
                ("state", self.random_state.as_str()),
            ])
            .build()
            .unwrap_or_else(|err| {
                eprintln!("Error building request: {}", err);
                std::process::exit(1);
            })
    }

    fn access_token_request(&self) -> Result<Request, AuthorizeError> {
        Ok(Client::new()
            .post(authorization_endpoint!("/api/token"))
            .form(&[
                ("grant_type", "authorization_code"),
                (
                    "code",
                    self.user_auth_code
                        .as_ref()
                        .ok_or(AuthorizeError::NoUserAuthCode)?
                        .as_str(),
                ),
                ("redirect_uri", self.redirect_url().as_str()),
            ])
            .basic_auth(self.client_id.as_str(), Some(self.client_secret.as_str()))
            .build()?)
    }
}

pub async fn get_access_token(
    config: Arc<Mutex<SpautofyConfig>>,
) -> Result<Access, AuthorizeError> {
    try_get_access_token(config, None).await
}

pub async fn try_get_access_token(
    config: Arc<Mutex<SpautofyConfig>>,
    old_access: Option<Access>,
) -> Result<Access, AuthorizeError> {
    let request = {
        let config = config.lock().unwrap();
        if config.user_auth_code.is_none() {
            return Err(AuthorizeError::NoUserAuthCode);
        }
        let try_refresh = match &old_access {
            Some(access) => access.is_expired(),
            None => true,
        };
        if !try_refresh {
            return old_access.ok_or(AuthorizeError::Unknown);
        }
        config.access_token_request()?
    };
    let resp = Client::new().execute(request).await?;
    let access = resp.json::<Access>().await;
    match access {
        Ok(access) => Ok(access),
        Err(_) => Err(AuthorizeError::ExpiredUserCode),
    }
}

#[get("/")]
pub fn index(config: &State<Arc<Mutex<SpautofyConfig>>>) -> Redirect {
    let config = config.lock().unwrap();
    if config.user_auth_code.is_some() {
        Redirect::to("/done")
    } else {
        Redirect::to("/auth")
    }
}

#[get("/done")]
pub fn done(
    config_filepath: &State<String>,
    config: &State<Arc<Mutex<SpautofyConfig>>>,
    shutdown: Shutdown,
) -> Result<&'static str, Redirect> {
    let config = config.lock().unwrap();
    if config.user_auth_code.is_none() {
        Err(Redirect::to("/auth"))
    } else {
        let file_config = SpautofyConfigFile::from(config.deref());
        let write_result = std::fs::write(
            config_filepath.as_str(),
            serde_json::to_string_pretty(&file_config).unwrap(),
        );
        if let Err(err) = write_result {
            eprintln!("Error writing config file: {}", err);
            exit(1);
        }
        shutdown.notify();
        Ok("You successfully authorized the app. The web server is going to stop. You can close this window now.")
    }
}

#[get("/auth")]
pub fn auth(config: &State<Arc<Mutex<SpautofyConfig>>>) -> Redirect {
    let config = config.lock().unwrap();
    let auth_req = config.auth_request();
    Redirect::to(auth_req.url().to_string())
}

#[get("/callback?<state>&<code>&<error>")]
pub fn callback(
    config: &State<Arc<Mutex<SpautofyConfig>>>,
    state: String,
    code: Option<String>,
    error: Option<String>,
) -> Redirect {
    let mut config = config.lock().unwrap();
    if state != config.random_state {
        eprintln!("Invalid state: {}", state);
        exit(1);
    }
    if let Some(error) = error {
        eprintln!("User Authentication Error: {}", error);
        exit(1);
    } else if code.is_some() {
        let mut config = config.deref_mut();
        config.user_auth_code = code;
    } else {
        eprintln!("Unexpected Error: No code or error returned from Spotify.");
        exit(1);
    }
    Redirect::to("/done")
}
