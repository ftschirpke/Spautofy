use clap::Parser;
use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::{Frame, Terminal};
use rocket::{routes, Config};
use std::fs;
use std::io;
use std::iter::zip;
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
    #[error("IO error: {0}")]
    IO(io::Error),
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

impl From<io::Error> for MainError {
    fn from(err: io::Error) -> Self {
        MainError::IO(err)
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

struct ActionSelectionList {
    action_names: Vec<&'static str>,
    selected: Vec<bool>,
    pub list_strings: Vec<String>,
    index: usize,
}

fn list_format(name: &str, is_selected: bool) -> String {
    format!("[{}] {}", if is_selected { 'X' } else { ' ' }, name)
}

impl ActionSelectionList {
    fn new(action_names: &'static [&str], selected: &[bool]) -> Self {
        let action_names = action_names.to_vec();
        let selected = selected.to_vec();
        let list_strings: Vec<_> = zip(action_names.iter(), selected.iter())
            .map(|(name, is_selected)| list_format(name, *is_selected))
            .collect();
        Self {
            action_names,
            selected,
            list_strings,
            index: 0,
        }
    }

    fn select(&mut self) {
        self.selected[self.index] ^= true;
        let is_selected = self.selected[self.index];
        self.list_strings[self.index] = list_format(self.action_names[self.index], is_selected);
    }

    fn previous(&mut self) {
        if self.index > 0 {
            self.index -= 1;
        }
    }

    fn next(&mut self) {
        if self.index < self.action_names.len() - 1 {
            self.index += 1;
        }
    }
}

static ACTION_NAMES: &[&str] = &[
    "Create playlist of your short term top tracks",
    "Create playlist of your medium term top tracks",
    "Create playlist of your long term top tracks",
];

static DEFAULT_SELECTION: &[bool] = &[true, true, false];

fn select_actions() -> Result<Option<ActionSelectionList>, io::Error> {
    let mut selection = ActionSelectionList::new(ACTION_NAMES, DEFAULT_SELECTION);

    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let mut canceled = false;
    let mut confirmed = false;
    while !canceled && !confirmed {
        terminal.draw(|frame| ui(frame, &selection))?;
        match handle_events(&mut selection)? {
            SelectionAction::Confirm => confirmed = true,
            SelectionAction::Cancel => canceled = true,
            SelectionAction::None => {}
        }
    }

    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;

    if canceled {
        Ok(None)
    } else if confirmed {
        Ok(Some(selection))
    } else {
        unreachable!()
    }
}

fn ui(frame: &mut Frame, action_selection_list: &ActionSelectionList) {
    let items = {
        let mut actions: Vec<ListItem> = action_selection_list
            .list_strings
            .iter()
            .enumerate()
            .map(|(i, name)| {
                if i != action_selection_list.index {
                    ListItem::new(name.as_str())
                } else {
                    ListItem::new(name.as_str()).style(Style::default().bg(Color::DarkGray))
                }
            })
            .collect();
        let mut additional = vec![
            ListItem::new("CONFIRM WITH 'SPACE'"),
            ListItem::new("CANCEL WITH 'ESC' OR 'q'"),
        ];
        actions.append(&mut additional);
        actions
    };
    let list = List::new(items)
        .block(
            Block::default()
                .title("Spautofy Actions")
                .borders(Borders::ALL),
        )
        .style(Style::default().fg(Color::White));
    frame.render_widget(list, frame.size());
}

enum TuiAction {
    Quit,
    Confirm,
    Select,
    MoveUp,
    MoveDown,
}

enum SelectionAction {
    Confirm,
    Cancel,
    None,
}

fn handle_events(action_selection_list: &mut ActionSelectionList) -> io::Result<SelectionAction> {
    if event::poll(std::time::Duration::from_millis(50))? {
        if let Event::Key(key) = event::read()? {
            if key.kind == event::KeyEventKind::Press {
                let tui_action = match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => TuiAction::Quit,
                    KeyCode::Char(' ') => TuiAction::Confirm,
                    KeyCode::Enter => TuiAction::Select,
                    KeyCode::Up | KeyCode::Char('k') => TuiAction::MoveUp,
                    KeyCode::Down | KeyCode::Char('j') => TuiAction::MoveDown,
                    _ => return Ok(SelectionAction::None),
                };
                match tui_action {
                    TuiAction::Quit => return Ok(SelectionAction::Cancel),
                    TuiAction::Confirm => return Ok(SelectionAction::Confirm),
                    TuiAction::Select => action_selection_list.select(),
                    TuiAction::MoveUp => action_selection_list.previous(),
                    TuiAction::MoveDown => action_selection_list.next(),
                }
            }
        }
    }
    Ok(SelectionAction::None)
}

#[rocket::main]
async fn main() -> Result<(), MainError> {
    let args = Args::parse();
    let selection = select_actions()?;
    let selection = match selection {
        Some(selection) => selection,
        None => {
            println!("Stopped Spautofy.");
            return Ok(());
        }
    };

    println!("Selected Actions:");
    selection
        .action_names
        .iter()
        .enumerate()
        .filter(|(i, _)| selection.selected[*i])
        .for_each(|(_, name)| println!("- {}", name));
    println!();

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

    if selection.selected[0] {
        create_top_track_playlist(&user_access, TimeRange::ShortTerm).await?;
    }
    if selection.selected[1] {
        create_top_track_playlist(&user_access, TimeRange::MediumTerm).await?;
    }
    if selection.selected[2] {
        create_top_track_playlist(&user_access, TimeRange::LongTerm).await?;
    }
    if selection.selected[3] {
        println!("Something else");
    }

    Ok(())
}
