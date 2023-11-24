#[macro_export]
macro_rules! authorization_endpoint {
    ( $( $x: expr),+ ) => {{
        format!("https://accounts.spotify.com{}", format_args!($($x),+))
    }};
}

#[macro_export]
macro_rules! api_endpoint {
    ( $( $x: expr),+ ) => {{
        format!("https://api.spotify.com/v1{}", format_args!($($x),+))
    }};
}
