use std::{collections::HashSet, path::PathBuf};

use axum::{
    Router,
    body::Body,
    extract::{Path, Query, Request},
    http::{HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Json, Response},
    routing::get,
};
use axum_response_cache::CacheLayer;
use hyper::header;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tower_http::compression::CompressionLayer;
use ulid::Ulid;

use crate::{
    assets::{ASSET_MANAGER, asset_routes},
    config::{Config, load_config},
    error_handler::error_handler,
    page::Page,
    security::add_security_headers,
};

#[derive(Debug, Deserialize)]
struct QueryParams {
    mode: Option<Mode>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all(deserialize = "lowercase"))]
enum Mode {
    Fragment,
    Edit,
}

#[derive(Debug, Serialize)]
#[serde(rename_all(serialize = "lowercase"))]
struct Fragment {
    id: Ulid,
    title: Option<String>,
    html: String,
    #[serde(with = "time::serde::iso8601")]
    modified: OffsetDateTime,
    tags: HashSet<String>,
}

pub async fn start_server() -> anyhow::Result<()> {
    let config = load_config();

    let compression_layer = CompressionLayer::new()
        .gzip(true)
        .deflate(true)
        .br(true)
        .zstd(true);

    let app = Router::new()
        .merge(asset_routes())
        .route("/", get(page_handler).layer(CacheLayer::with_lifespan(1)))
        .route(
            "/{*path}",
            get(page_handler).layer(CacheLayer::with_lifespan(1)),
        )
        .layer(middleware::from_fn(error_handler))
        .layer(middleware::from_fn(add_security_headers))
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
) -> Result<impl IntoResponse, StatusCode> {
    let config = load_config();
    let page = page_content(path, &config)?;

    if query.mode == Some(Mode::Fragment) {
        let fragment = Fragment {
            id: page.id,
            title: page.title,
            html: format!("<main><article>{}</article></main>", page.html),
            modified: page.modified,
            tags: page.tags,
        };
        Ok(Json(&fragment).into_response())
    } else {
        Ok(Html(full_page_html(&page, &config)).into_response())
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
            ::view-transition-new(root),
            ::view-transition-old(article),
            ::view-transition-new(article) {{
                animation-duration: 50ms;
                animation-timing-function: ease-in-out;
            }}

            article {{
                view-transition-name: article;
            }}
        </style>
        <link rel="stylesheet" href="{}">
        <script type="module" src="{}"></script>
    </head>
    <body>
        <main>
            <article>{}</article>
        </main>
    </body>
</html>"#,
        formulate_title(page, config),
        ASSET_MANAGER.hashed_route("styles.css").unwrap_or_default(),
        ASSET_MANAGER.hashed_route("script.js").unwrap_or_default(),
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
    if let Some(content_type) = response.headers().get(header::CONTENT_TYPE) {
        if content_type == "text/html" {
            response
                .headers_mut()
                .insert("View-Transition", HeaderValue::from_static("same-origin"));
        }
    }
    response
}
