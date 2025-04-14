use std::{collections::HashSet, sync::Arc};

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
use tokio::sync::RwLock;
use tower_http::compression::CompressionLayer;
use ulid::Ulid;

use crate::{
    assets::{ASSET_MANAGER, asset_routes},
    config::{Config, load_config},
    error_handler::error_handler,
    page::Page,
    search::{SearchIndex, search_route},
    security::add_security_headers,
};

#[derive(Debug, Deserialize)]
struct QueryParams {
    q: Option<String>,
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

pub async fn start_server(
    config: &Config,
    search_index: Arc<RwLock<SearchIndex>>,
) -> anyhow::Result<()> {
    let compression_layer = CompressionLayer::new()
        .gzip(true)
        .deflate(true)
        .br(true)
        .zstd(true);

    let app = Router::new()
        .merge(asset_routes())
        .merge(search_route(search_index))
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
    let path = path.unwrap_or(Path("/".into())).0;
    let page = Page::read(path).map_err(|_| StatusCode::NOT_FOUND)?;

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
        Ok(Html(full_page_html(&page, query.q)).into_response())
    }
}

fn full_page_html(page: &Page, query: Option<String>) -> String {
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
            <search>
                <form method="get" action="/search">
                    <label for="search">
                    <input id="search" type="search" name="q" value="{}">
                    <button>Search</button>
                </form>
            </search>
            <article>{}</article>
        </main>
    </body>
</html>"#,
        formulate_title(page),
        ASSET_MANAGER.hashed_route("styles.css").unwrap_or_default(),
        ASSET_MANAGER.hashed_route("script.js").unwrap_or_default(),
        &query.unwrap_or_default(),
        &page.html
    )
}

fn formulate_title(page: &Page) -> String {
    let config = load_config();
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
