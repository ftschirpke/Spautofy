use serde::{Deserialize, Serialize};

use crate::models::artist::Artist;

#[derive(Debug, Deserialize, Serialize)]
pub struct Album {
    id: String,
    name: String,
    album_type: String,
    artists: Vec<Artist>,
    total_tracks: i32,
    release_date: String,
}
