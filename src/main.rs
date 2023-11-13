use clap::Parser;
use rocket::{routes, Config};
use std::fs::{self};
use std::path::Path;
use std::sync::Mutex;

mod authorize;

use self::authorize::{auth, callback, done, index, SpautofyConfig, SpautofyConfigFile};

extern crate rocket;

#[derive(Debug, Parser)]
#[command(version, author, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "spautofy.config")]
    config_path: String,
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
        \tport = <port>,                       // optional - port for the web app (default: 3000)\n\
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

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    let args = Args::parse();
    let file_config = parse_config_file(args.config_path.as_str());
    let config = SpautofyConfig::from(file_config);

    if config.needs_auth() {
        println!("You need to authenticate with Spotify.");
        println!("Please visit the following URL in your browser");

        let rocket_config = Config {
            address: config.address,
            port: config.port,
            ..Config::release_default()
        };
        let rocket = rocket::custom(&rocket_config)
            .manage(args.config_path)
            .manage(Mutex::new(config))
            .mount("/", routes![index, auth, callback, done])
            .ignite()
            .await?;
        rocket.launch().await?;
        println!("Stopped the web server.");
    } else {
        println!("Already authenticated, skipping web server.");
    }
    println!("The app is ready to make user-authenticated requests.");

    Ok(())
}
