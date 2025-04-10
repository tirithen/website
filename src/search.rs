use std::{
    io::Cursor,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Result;
use axum::{Router, extract::Query, response::Html, routing::get};
use heed::EnvOpenOptions;
use milli::{
    Index, Search, SearchResult,
    documents::{DocumentsBatchBuilder, DocumentsBatchReader},
    update::{ClearDocuments, IndexDocuments, IndexDocumentsConfig, IndexerConfig, Settings},
};
use serde::Deserialize;
use serde_json::Value;
use tokio::time::interval;

use crate::{config::Config, page::Page};

#[derive(Deserialize)]
struct SearchParams {
    q: String,
}

pub fn spawn_search_indexer(config: &Config) -> Result<SearchIndex> {
    let search_index = SearchIndex::new(&config.search_path())?;
    let mut search_index_background = search_index.clone();

    tokio::spawn(async move {
        if let Err(e) = search_index_background.reindex().await {
            tracing::error!("💥 Initial reindex failed: {}", e);
        }

        let mut interval = interval(Duration::from_secs(30 * 60));
        loop {
            interval.tick().await;
            if let Err(e) = search_index_background.reindex().await {
                tracing::error!("💥 Periodic reindex failed: {}", e);
            }
        }
    });

    Ok(search_index)
}

fn create_or_open_index(path: &Path) -> Result<Index> {
    std::fs::create_dir_all(path)?;

    let mut options = EnvOpenOptions::new();
    options.map_size(100 * 1024 * 1024);
    options.max_dbs(1);
    let options = options.read_txn_without_tls();
    let index = Index::new(options, path, true)?;

    let mut wtxn = index.write_txn()?;
    let config = IndexerConfig::default();
    let mut builder = Settings::new(&mut wtxn, &index, &config);
    builder.set_searchable_fields(vec!["title".into(), "markdown".into()]);
    builder.execute(|_| (), || false)?;
    wtxn.commit()?;

    Ok(index)
}

#[derive(Clone)]
pub struct SearchIndex {
    active_index: Index,
    staging_index: Index,
    active_path: PathBuf,
    staging_path: PathBuf,
}

impl SearchIndex {
    pub fn new(path: &Path) -> Result<Self> {
        let active_path = path.join("active");
        let staging_path = path.join("staging");

        let active_index = create_or_open_index(&active_path)?;
        let staging_index = create_or_open_index(&staging_path)?;

        Ok(Self {
            active_index,
            staging_index,
            active_path,
            staging_path,
        })
    }

    pub async fn reindex(&mut self) -> Result<()> {
        tracing::info!("🔎 Re-creating index");
        tracing::debug!("sdfdsf");
        self.clear_staging()?;
        for page in Page::all() {
            self.index_page_to(&page, &self.staging_index)?;
        }
        self.swap_indicies()?;
        Ok(())
    }

    pub fn search_route(&self) -> Router {
        let cloned_self = self.clone();
        Router::new().route(
            "/search",
            get(async move |Query(params): Query<SearchParams>| {
                let query = params.q;
                let results = cloned_self.search(&query).unwrap_or_else(|_| Vec::new());
                render_search_results(results)
            }),
        )
    }

    pub fn search(&self, query: &str) -> Result<Vec<serde_json::Value>> {
        tracing::debug!("Searching with query: {}", query);
        let rtxn = self.active_index.read_txn()?;

        let mut search = Search::new(&rtxn, &self.active_index);
        search.query(query);
        search.limit(10);

        let results: SearchResult = search.execute()?;
        let documents = self.active_index.documents(&rtxn, results.documents_ids)?;
        let fields_map = self.active_index.fields_ids_map(&rtxn)?;

        let mut output = Vec::new();
        for (_id, obkv_doc) in documents.iter() {
            let mut doc = serde_json::Map::new();
            for (field_id, value_bytes) in obkv_doc.iter() {
                let field_name = if let Some(name) = fields_map.name(field_id) {
                    name
                } else {
                    continue;
                };

                let value: Value = serde_json::from_slice(value_bytes)?;

                doc.insert(field_name.to_string(), value);
            }
            output.push(Value::Object(doc));
        }

        Ok(output)
    }

    pub fn index_page(&self, page: &Page) -> Result<()> {
        self.index_page_to(page, &self.active_index)
    }

    fn swap_indicies(&mut self) -> Result<()> {
        tracing::debug!("Swapping indicies",);
        let old_path = self.active_path.parent().unwrap().join("old");
        std::fs::rename(&self.active_path, &old_path)?;
        std::fs::rename(&self.staging_path, &self.active_path)?;
        std::fs::rename(&old_path, &self.staging_path)?;
        self.active_index = create_or_open_index(&self.active_path)?;
        self.staging_index = create_or_open_index(&self.staging_path)?;
        Ok(())
    }

    fn clear_staging(&self) -> Result<()> {
        tracing::debug!("Clear out staging");
        let mut wtxn = self.staging_index.write_txn()?;
        let clear = ClearDocuments::new(&mut wtxn, &self.staging_index);
        clear.execute()?;
        wtxn.commit()?;
        Ok(())
    }

    fn index_page_to(&self, page: &Page, index: &Index) -> Result<()> {
        tracing::debug!(
            "Indexing page {}",
            page.title.clone().unwrap_or(page.id.to_string())
        );
        let mut wtxn = index.write_txn()?;
        let config = IndexerConfig::default();
        let indexing_config = IndexDocumentsConfig::default();

        let mut batch = DocumentsBatchBuilder::new(Vec::new());
        batch.append_json_object(
            serde_json::json!({
                "id": page.id.to_string(),
                "title": page.title,
                "markdown": page.markdown,
                "modified": page.modified,
                "url": page.url,
                "tags": page.tags
            })
            .as_object()
            .unwrap(),
        )?;

        let vector = batch.into_inner().unwrap();
        let reader = DocumentsBatchReader::from_reader(Cursor::new(vector))?;
        let builder =
            IndexDocuments::new(&mut wtxn, index, &config, indexing_config, |_| (), || false)?;

        let (builder, _) = builder.add_documents(reader)?;
        builder.execute()?;
        wtxn.commit()?;

        Ok(())
    }
}

fn render_search_results(results: Vec<serde_json::Value>) -> Html<String> {
    let mut html =
        String::from("<!DOCTYPE html><html><head><title>Search Results</title></head><body>");
    html.push_str("<h1>Search Results</h1><ul>");

    for result in results {
        let title = result
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled");
        let url = result.get("url").and_then(|v| v.as_str()).unwrap_or("#");
        html.push_str(&format!("<li><a href=\"{}\">{}</a></li>", url, title));
    }

    html.push_str("</ul></body></html>");
    Html(html)
}
