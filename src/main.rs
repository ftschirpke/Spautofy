use clap::parser::MatchesError;
use clap::{Arg, Command};

struct Args {
    config_path: String,
}

fn parse_cli_args() -> Result<Args, MatchesError> {
    let cmd = Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::new("config")
                .short('c')
                .required(false)
                .default_value("spautofy.config"),
        )
        .get_matches();

    let args = Args {
        config_path: cmd.try_get_one::<String>("config")?.unwrap().to_string(),
    };
    Ok(args)
}

fn main() {
    let args = parse_cli_args();
}
