use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::authorize::{Access, AuthorizeError};
use crate::{api_endpoint, UserAccess};

#[derive(Debug, Deserialize, Serialize)]
pub struct User {
    pub display_name: String,
    pub id: String,
}

pub async fn get_user_access(access: Access) -> Result<UserAccess, AuthorizeError> {
    let user = get_user_info(&access).await?;
    Ok(UserAccess { access, user })
}

async fn get_user_info(access: &Access) -> Result<User, AuthorizeError> {
    let client = Client::new();
    let request_builder = client.get(api_endpoint!("/me"));
    let request_builder = access.authorize(request_builder);
    let request = request_builder.build()?;
    let resp = client.execute(request).await?;
    let resp = resp.json::<User>().await?;
    Ok(resp)
}
