use axum::{
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};

use crate::{assets::ASSET_MANAGER, config::load_config};

pub async fn error_handler(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<Response, (StatusCode, String)> {
    let response = next.run(request).await;

    if response.status().is_client_error() || response.status().is_server_error() {
        let status = response.status();
        let html = render_error_page(status);
        return Ok(html.into_response());
    }

    Ok(response)
}

fn render_error_page(status: StatusCode) -> Response {
    let config = load_config();
    let title = format!(
        "{} {}",
        status.as_str(),
        status.canonical_reason().unwrap_or("Error")
    );
    let html = format!(
        r#"<!DOCTYPE html>
<html>
    <head>
        <meta http-equiv="Content-Type" content="text/html; charset=UTF-8">
        <meta http-equiv="X-UA-Compatible" content="IE=Edge">
        <meta name="viewport" content="width=device-width,initial-scale=1">
        <title>{} - {}</title>
        <link rel="stylesheet" href="{}">
    </head>
    <body>
        <main class="error-page error-page--{}">
            <h1>{}</h1>
            <p><a href="/">To start page</a></p>
        </main>
    </body>
</html>"#,
        title,
        config.title(),
        ASSET_MANAGER.hashed_route("styles.css").unwrap_or_default(),
        status.as_str(),
        title
    );

    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(html.into())
        .unwrap()
}
