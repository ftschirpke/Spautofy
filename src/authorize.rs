use std::net::{IpAddr, Ipv4Addr};
use std::ops::Deref;
use std::process::exit;
use std::sync::Mutex;

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use reqwest::{Client, Request};
use rocket::response::Redirect;
use rocket::{get, Shutdown, State};
use serde::{Deserialize, Serialize};

const AUTHORIZATION_SCOPES: &str = "user-top-read playlist-read-private playlist-modify-private";

#[derive(Debug, Deserialize, Serialize)]
pub struct SpautofyConfigFile {
    address: Option<IpAddr>,
    port: Option<u16>,
    client_id: String,
    client_secret: String,
    user_auth_code: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SpautofyConfig {
    pub address: IpAddr,
    pub port: u16,
    client_id: String,
    client_secret: String,
    user_auth_code: Option<String>,
    random_state: String,
}

impl From<&SpautofyConfig> for SpautofyConfigFile {
    fn from(config: &SpautofyConfig) -> Self {
        SpautofyConfigFile {
            address: Some(config.address),
            port: Some(config.port),
            client_id: config.client_id.clone(),
            client_secret: config.client_secret.clone(),
            user_auth_code: config.user_auth_code.clone(),
        }
    }
}

fn random_state() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect()
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
            user_auth_code: file_config.user_auth_code,
            random_state: random_state(),
        }
    }
}

impl SpautofyConfig {
    pub fn needs_auth(&self) -> bool {
        self.user_auth_code.is_none()
    }

    fn redirect_url(&self) -> String {
        format!("http://{}:{}/callback", self.address, self.port)
    }

    fn client(&self) -> Client {
        Client::builder().build().unwrap_or_else(|err| {
            eprintln!("Error building client: {}", err);
            std::process::exit(1);
        })
    }

    fn auth_request(&self) -> Request {
        self.client()
            .get("https://accounts.spotify.com/authorize")
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
}

#[get("/")]
pub fn index(config: &State<Mutex<SpautofyConfig>>) -> Redirect {
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
    config: &State<Mutex<SpautofyConfig>>,
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
pub fn auth(config: &State<Mutex<SpautofyConfig>>) -> Redirect {
    let config = config.lock().unwrap();
    let auth_req = config.auth_request();
    Redirect::to(auth_req.url().to_string())
}

#[get("/callback?<state>&<code>&<error>")]
pub fn callback(
    config: &State<Mutex<SpautofyConfig>>,
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
        config.user_auth_code = code;
    } else {
        eprintln!("Unexpected Error: No code or error returned from Spotify.");
        exit(1);
    }
    Redirect::to("/done")
}
