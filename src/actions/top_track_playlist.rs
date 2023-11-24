use chrono::Local;
use reqwest::Client;
use serde::Deserialize;
use std::fmt::Display;

use crate::actions::playlist_actions::create_private_playlist;
use crate::authorize::AuthorizeError;
use crate::models::track::Track;
use crate::{api_endpoint, UserAccess};

use super::playlist_actions::update_playlist_tracks;

#[derive(Debug)]
pub enum TimeRange {
    ShortTerm,
    MediumTerm,
    LongTerm,
}

impl Display for TimeRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeRange::ShortTerm => write!(f, "short_term"),
            TimeRange::MediumTerm => write!(f, "medium_term"),
            TimeRange::LongTerm => write!(f, "long_term"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct TopTracksResponse {
    href: String,
    limit: i32,
    offset: i32,
    total: i32,
    next: Option<String>,
    previous: Option<String>,
    items: Vec<Track>,
}

pub async fn create_top_track_playlist(
    user_access: &UserAccess,
    time_range: TimeRange,
) -> Result<(), AuthorizeError> {
    let client = Client::new();
    let request_builder = client.get(api_endpoint!("/me/top/tracks"));
    let request_builder = user_access.access.authorize(request_builder);
    let request = request_builder
        .query(&[
            ("time_range", time_range.to_string().as_str()),
            ("limit", "50"),
        ])
        .build()?;
    let resp = client.execute(request).await?;
    let resp = resp.json::<TopTracksResponse>().await?;

    let date_today = Local::now().format("%d-%m-%Y").to_string();
    let playlist_name = format!("Spautofy {} Top Tracks {}", time_range, date_today);
    let playlist = create_private_playlist(user_access, &playlist_name).await?;

    let track_uris: Vec<&str> = resp.items.iter().map(|track| track.uri.as_str()).collect();
    update_playlist_tracks(user_access, &playlist.id, &track_uris).await?;

    println!("Created playlist \"{}\", enjoy!", playlist.name);

    Ok(())
}
