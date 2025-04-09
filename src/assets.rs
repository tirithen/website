use std::collections::HashMap;

use axum::{Router, http::HeaderValue};
use hyper::header;
use lazy_static::lazy_static;
use rust_embed::Embed;
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer};

include!("../target/generated_asset_manifest.rs");

lazy_static! {
    pub static ref ASSET_MANAGER: AssetManager = {
        let manifest = ASSET_MANIFEST
            .entries()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        AssetManager::new(manifest)
    };
}

pub fn asset_routes() -> Router {
    let static_service =
        ServeDir::new("target/assets_hashed").append_index_html_on_directories(false);

    Router::new().nest_service("/assets", static_service).layer(
        SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        ),
    )
}

#[derive(Embed)]
#[folder = "target/assets_hashed/"]
struct EmbeddedAssets;

#[derive(Debug, Clone)]
pub struct AssetManager {
    manifest: HashMap<String, String>,
}

impl AssetManager {
    fn new(manifest: HashMap<String, String>) -> Self {
        Self { manifest }
    }

    pub fn hashed_route(&self, original_path: &str) -> Option<String> {
        let asset = self.manifest.get(original_path);
        asset.map(|a| format!("/assets/{a}"))
    }
}
