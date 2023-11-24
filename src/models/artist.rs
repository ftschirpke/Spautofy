use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct SimplifiedArtist {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Artist {
    id: String,
    name: String,
    genres: Option<Vec<String>>,
    popularity: Option<i32>,
}

impl From<Artist> for SimplifiedArtist {
    fn from(artist: Artist) -> Self {
        SimplifiedArtist {
            id: artist.id,
            name: artist.name,
        }
    }
}
