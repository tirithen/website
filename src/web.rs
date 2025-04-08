use axum::{
    Router,
    extract::{Path, Query},
    http::StatusCode,
    response::Html,
    routing::get,
};
use serde::Deserialize;

use crate::{
    config::{Config, load_config},
    page::Page,
};

#[derive(Deserialize, Debug)]
struct QueryParams {
    mode: Option<Mode>,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all(deserialize = "lowercase"))]
enum Mode {
    Partial,
    Edit,
}

pub async fn start_server() -> anyhow::Result<()> {
    let config = load_config();
    let app = Router::new()
        .route("/", get(page_handler))
        .route("/{*path}", get(page_handler));
    let address = format!("0.0.0.0:{}", config.port());
    let listener = tokio::net::TcpListener::bind(&address).await?;

    tracing::info!("🚀 Starting website server at: http://{address}");
    axum::serve(listener, app).await?;

    Ok(())
}

async fn page_handler(
    path: Option<Path<String>>,
    Query(query): Query<QueryParams>,
) -> Result<Html<String>, StatusCode> {
    let config = load_config();
    let page = page_content(path, &config)?;

    if query.mode == Some(Mode::Partial) {
        Ok(Html(page.html))
    } else {
        Ok(Html(full_page_html(&page, &config)))
    }
}

fn page_content(url_path: Option<Path<String>>, config: &Config) -> Result<Page, StatusCode> {
    let mut path = url_path.unwrap_or(Path("/".into()));
    if path.ends_with("/") {
        path.push_str("index.md");
    } else {
        path.push_str(".md");
    }
    path = Path(
        path.strip_prefix("/")
            .map(|p| p.into())
            .unwrap_or(path.clone()),
    );

    let pages_root = config.data_path().join("pages");
    let file_path = pages_root
        .join(path.to_string())
        .canonicalize()
        .map_err(|_| StatusCode::NOT_FOUND)?;

    if !file_path.starts_with(&pages_root) {
        return Err(StatusCode::BAD_REQUEST);
    }

    Page::read(&file_path).map_err(|_| StatusCode::NOT_FOUND)
}

fn full_page_html(page: &Page, config: &Config) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
    <head>
        <meta http-equiv="Content-Type" content="text/html; charset=UTF-8">
        <meta http-equiv="X-UA-Compatible" content="IE=Edge">
        <meta name="viewport" content="width=device-width,initial-scale=1">
        <title>{}</title>
    </head>
    <body>
        <main>
            {}
        </main>
    </body>
</html>"#,
        formulate_title(page, config),
        &page.html
    )
}

fn formulate_title(page: &Page, config: &Config) -> String {
    if let Some(page_title) = &page.title {
        format!("{} - {}", page_title, config.title())
    } else {
        config.title().clone()
    }
}
