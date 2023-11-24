use clap::Parser;
use rocket::{routes, Config};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use thiserror::Error;

mod actions;
mod authorize;
mod endpoints;
mod models;
mod user_info;

use actions::top_track_playlist::{create_top_track_playlist, TimeRange};
use authorize::{
    auth, callback, done, get_access_token, index, Access, AuthorizeError, SpautofyConfig,
    SpautofyConfigFile,
};
use user_info::{get_user_access, User};

extern crate rocket;

#[derive(Debug, Parser)]
#[command(version, author, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "spautofy.config")]
    config_path: String,
}

#[derive(Debug, Error)]
enum MainError {
    #[error("Authorization error: {0}")]
    Auth(AuthorizeError),
    #[error("Rocket error: {0}")]
    Rocket(rocket::Error),
    #[error("Unknown error.")]
    Unknown,
}

impl From<AuthorizeError> for MainError {
    fn from(err: AuthorizeError) -> Self {
        MainError::Auth(err)
    }
}

impl From<rocket::Error> for MainError {
    fn from(err: rocket::Error) -> Self {
        MainError::Rocket(err)
    }
}

#[derive(Debug)]
pub struct UserAccess {
    pub access: Access,
    pub user: User,
}

impl UserAccess {
    pub fn authorize(&self, request_builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        self.access.authorize(request_builder)
    }
}

fn parse_config_file(filepath_str: &str) -> SpautofyConfigFile {
    let path = Path::new(filepath_str);
    if !path.exists() {
        eprintln!("Config file \"{}\" does not exist.", filepath_str);
        eprintln!(
        "Please create a config file with the following format:\n\
        {{\n\
        \tclient_id = \"<client_id>\",         // required - get this from https://developer.spotify.com/dashboard\n\
        \tclient_secret = \"<client_secret>\", // required - get this from https://developer.spotify.com/dashboard\n\
        \taddress = \"<address>\",             // optional - address for the web app (default: \"127.0.0.1\")\n\
        \tport = <port>,                     // optional - port for the web app (default: 3000)\n\
        }}"
        );
        std::process::exit(1);
    }
    let config = fs::read_to_string(filepath_str).unwrap_or_else(|err| {
        eprintln!("Error reading config file {}: {}", filepath_str, err);
        std::process::exit(1);
    });
    serde_json::from_str::<SpautofyConfigFile>(&config).unwrap_or_else(|err| {
        eprintln!("Error parsing config file {}: {}", filepath_str, err);
        std::process::exit(1);
    })
}

async fn user_authorization(
    args: &Args,
    config: Arc<Mutex<SpautofyConfig>>,
) -> Result<(), rocket::Error> {
    println!("You need to authenticate with Spotify.");
    println!("Please visit the following URL in your browser");

    let rocket_config = {
        let unwrapped_config = config.lock().unwrap();
        Config {
            address: unwrapped_config.address,
            port: unwrapped_config.port,
            ..Config::release_default()
        }
    };
    let rocket = rocket::custom(&rocket_config)
        .manage(args.config_path.clone())
        .manage(config.clone())
        .mount("/", routes![index, auth, callback, done])
        .ignite()
        .await?;
    rocket.launch().await?;
    println!("Stopped the web server.");
    Ok(())
}

async fn authorize(
    args: &Args,
    file_config: SpautofyConfigFile,
) -> Result<(SpautofyConfig, UserAccess), MainError> {
    let config = Arc::new(Mutex::new(SpautofyConfig::from(file_config)));
    user_authorization(args, config.clone()).await?;

    println!("Getting access token...");
    let access = get_access_token(config.clone()).await?;
    let user_access = get_user_access(access).await?;
    let lock = Arc::try_unwrap(config).expect("Arc has multiple owners");
    let config = lock.into_inner().expect("Mutex is already unlocked");
    Ok((config, user_access))
}

#[rocket::main]
async fn main() -> Result<(), MainError> {
    let args = Args::parse();
    let file_config = parse_config_file(args.config_path.as_str());

    let (config, user_access) = authorize(&args, file_config).await?;
    let _ = std::fs::write(
        args.config_path.as_str(),
        serde_json::to_string_pretty(&config).expect("Failed to serialize config"),
    );
    println!(
        "Successfully authenticated with Spotify as user {}.",
        user_access.user.display_name
    );

    println!("Creating top track playlist");
    create_top_track_playlist(&user_access, TimeRange::ShortTerm).await?;
    create_top_track_playlist(&user_access, TimeRange::MediumTerm).await?;
    create_top_track_playlist(&user_access, TimeRange::LongTerm).await?;

    Ok(())
}
