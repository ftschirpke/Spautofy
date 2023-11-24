use serde::{Deserialize, Serialize};

use crate::models::track::Track;

#[derive(Debug, Deserialize, Serialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub description: String,
    pub collaborative: bool,
    pub href: String,
    pub public: bool,
    pub tracks: PlaylistItems,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PlaylistItems {
    pub href: String,
    pub total: i32,
    pub offset: i32,
    pub next: Option<String>,
    pub previous: Option<String>,
    pub items: Vec<Track>,
}
