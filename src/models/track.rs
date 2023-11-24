use serde::{Deserialize, Serialize};

use crate::models::album::Album;
use crate::models::artist::Artist;

#[derive(Debug, Deserialize, Serialize)]
pub struct Track {
    pub id: String,
    pub uri: String,
    pub name: String,
    pub album: Album,
    pub artists: Vec<Artist>,
}
