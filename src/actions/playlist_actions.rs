use reqwest::Client;
use serde_json::json;

use crate::authorize::AuthorizeError;
use crate::models::playlist::{Playlist, PlaylistItems};
use crate::{api_endpoint, UserAccess};

pub async fn create_playlist(
    user_access: &UserAccess,
    name: &str,
    public: bool,
    description: Option<&str>,
    collaborative: bool,
) -> Result<Playlist, AuthorizeError> {
    let client = Client::new();
    let user_id = &user_access.user.id;
    let request_builder = client.post(api_endpoint!("/users/{user_id}/playlists"));
    let request_builder = user_access.authorize(request_builder);
    let request = request_builder
        .body(
            json!({
                "name": name,
                "public": public,
                "description": description.unwrap_or_default(),
                "collaborative": collaborative,
            })
            .to_string(),
        )
        .build()?;
    let resp = client.execute(request).await?;
    let resp = resp.json::<Playlist>().await?;
    Ok(resp)
}

pub async fn create_private_playlist(
    user_access: &UserAccess,
    name: &str,
) -> Result<Playlist, AuthorizeError> {
    create_playlist(user_access, name, false, None, false).await
}

pub async fn add_50_to_playlist(
    user_access: &UserAccess,
    playlist_id: &str,
    track_uris: &[&str],
) -> Result<(), AuthorizeError> {
    let client = Client::new();
    let request_builder = client.post(api_endpoint!("/playlists/{playlist_id}/tracks"));
    let request_builder = user_access.authorize(request_builder);
    let request = request_builder
        .body(json!({ "uris": track_uris }).to_string())
        .build()?;
    let _resp = client.execute(request).await?;
    Ok(())
}

pub async fn update_playlist_tracks(
    user_access: &UserAccess,
    playlist_id: &str,
    track_uris: &[&str],
) -> Result<(), AuthorizeError> {
    let client = Client::new();
    let request_builder = client.put(api_endpoint!("/playlists/{playlist_id}/tracks"));
    let request_builder = user_access.authorize(request_builder);
    let request = request_builder
        .body(json!({ "uris": track_uris }).to_string())
        .build()?;
    let _resp = client.execute(request).await?;
    Ok(())
}

pub async fn get_playlist_tracks(
    user_access: &UserAccess,
    playlist_id: &str,
) -> Result<PlaylistItems, AuthorizeError> {
    let client = Client::new();
    let request_builder = client.get(api_endpoint!("/playlists/{playlist_id}/tracks"));
    let request_builder = user_access.authorize(request_builder);
    let request = request_builder.build()?;
    let resp = client.execute(request).await?;
    let resp = resp.json::<PlaylistItems>().await?;
    Ok(resp)
}
