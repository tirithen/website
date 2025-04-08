use axum::{
    Router,
    body::Body,
    extract::{Path, Query, Request},
    http::{HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{Html, Response},
    routing::get,
};
use axum_response_cache::CacheLayer;
use serde::Deserialize;
use tower_http::compression::CompressionLayer;

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

    let compression_layer = CompressionLayer::new()
        .gzip(true)
        .deflate(true)
        .br(true)
        .zstd(true);

    let app = Router::new()
        .route("/", get(page_handler).layer(CacheLayer::with_lifespan(1)))
        .route(
            "/{*path}",
            get(page_handler).layer(CacheLayer::with_lifespan(1)),
        )
        .layer(middleware::from_fn(add_performance_headers))
        .layer(compression_layer);
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
        <style>
            @view-transition {{
                navigation: auto;
            }}

            ::view-transition-old(root),
            ::view-transition-new(root) {{
                animation-duration: 50ms;
                animation-timing-function: ease-in-out;
            }}
        </style>
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

async fn add_performance_headers(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert("View-Transition", HeaderValue::from_static("same-origin"));
    response
}
