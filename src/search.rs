use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use anyhow::Result;
use axum::{Router, extract::Query, response::Html, routing::get};
use heed::EnvOpenOptions;
use milli::{
    DefaultSearchLogger, FormatOptions, GeoSortStrategy, Index, MatcherBuilder, MatchingWords,
    SearchContext, TermsMatchingStrategy, TimeBudget,
    documents::{DocumentsBatchBuilder, DocumentsBatchReader},
    execute_search, filtered_universe,
    score_details::ScoringStrategy,
    tokenizer::TokenizerBuilder,
    update::{ClearDocuments, IndexDocuments, IndexDocumentsConfig, IndexerConfig, Settings},
};
use rayon::iter::ParallelIterator;
use serde::Deserialize;
use serde_json::Value;
use tokio::{sync::RwLock, time::interval};

use crate::{assets::ASSET_MANAGER, config::Config, page::Page};

pub async fn spawn_search_indexer(config: &Config) -> Result<Arc<RwLock<SearchIndex>>> {
    let search_index = Arc::new(RwLock::new(SearchIndex::new(&config.search_path())?));
    let search_index_background = search_index.clone();
    let duration = *config.search_reindex_interval();

    tokio::spawn(async move {
        let mut interval = interval(duration);
        interval.tick().await;

        loop {
            if let Err(e) = search_index_background.read().await.reindex().await {
                tracing::error!("💥 Periodic reindex failed: {}", e);
            }
            if let Err(e) = search_index_background.write().await.swap_indexes().await {
                tracing::error!("💥 Swapping indexes failed: {}", e);
            }
            interval.tick().await;
        }
    });

    Ok(search_index)
}

pub fn search_route(search_index: Arc<RwLock<SearchIndex>>) -> Router {
    let search_index = search_index;
    Router::new().route(
        "/search",
        get(async move |Query(params): Query<SearchParams>| {
            let query = params.q;
            let results = search_index
                .read()
                .await
                .search(&query)
                .await
                .unwrap_or_else(|_| Vec::new());
            render_search_results(query, results)
        }),
    )
}

pub struct SearchIndex {
    active_index: Index,
    staging_index: Index,
    active_path: PathBuf,
    staging_path: PathBuf,
    alpha_path: PathBuf,
    beta_path: PathBuf,
}

impl SearchIndex {
    pub fn new(path: &Path) -> Result<Self> {
        let active_path = path.join("active");
        let staging_path = path.join("staging");
        let alpha_path = path.join("alpha");
        let beta_path = path.join("beta");

        fs::create_dir_all(&alpha_path)?;
        fs::create_dir_all(&beta_path)?;

        if !active_path.exists() {
            symlink(&alpha_path, &active_path)?;
            let _ = fs::remove_file(&staging_path);
            symlink(&beta_path, &staging_path)?;
        }

        let active_index = create_or_open_index(&active_path)?;
        let staging_index = create_or_open_index(&staging_path)?;

        Ok(Self {
            active_index,
            staging_index,
            active_path,
            staging_path,
            alpha_path,
            beta_path,
        })
    }

    pub async fn search(&self, query: &str) -> Result<Vec<serde_json::Value>> {
        tracing::debug!("Searching with query: {}", query);
        let rtxn = self.active_index.read_txn()?;
        let mut ctx = SearchContext::new(&self.active_index, &rtxn)?;
        let universe = filtered_universe(ctx.index, ctx.txn, &None)?;
        let search_result = execute_search(
            &mut ctx,
            Some(query),
            TermsMatchingStrategy::Last,
            ScoringStrategy::Detailed,
            false,
            universe,
            &None,
            &None,
            GeoSortStrategy::default(),
            0,
            10,
            Some(10),
            &mut DefaultSearchLogger,
            &mut DefaultSearchLogger,
            TimeBudget::default(),
            None,
            None,
        )?;

        let document_ids = search_result.documents_ids;

        let matching_words =
            MatchingWords::new(ctx, search_result.located_query_terms.unwrap_or_default());
        let tokenizer = TokenizerBuilder::default().into_tokenizer();
        let mut matcher_builder = MatcherBuilder::new(matching_words, tokenizer);
        matcher_builder.highlight_prefix("<mark>".to_string());
        matcher_builder.highlight_suffix("</mark>".to_string());

        let format_options = FormatOptions {
            highlight: true,
            crop: Some(20),
        };

        let documents = self.active_index.documents(&rtxn, document_ids)?;
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

                if field_name == "markdown" && value.is_string() {
                    let text = value.as_str().unwrap();
                    let mut matcher = matcher_builder.build(text, None);
                    let formatted_text = matcher.format(format_options);

                    doc.insert(field_name.to_string(), value.clone());
                    doc.insert(
                        "_formatted_markdown".to_string(),
                        Value::String(formatted_text.into_owned()),
                    );
                } else {
                    doc.insert(field_name.to_string(), value);
                }
            }

            output.push(Value::Object(doc));
        }

        Ok(output)
    }

    pub async fn index_page(&self, page: Page) -> Result<()> {
        self.commit_batch(vec![page], &self.active_index).await
    }

    async fn reindex(&self) -> Result<()> {
        tracing::info!("🔎 Indexing all pages...");
        let start = SystemTime::now();

        self.clear_staging().await?;

        let (tx, mut rx) = tokio::sync::mpsc::channel(1000);

        let producer = tokio::task::spawn_blocking(move || {
            Page::all()
                .filter_map(|page| {
                    let _ = tx.blocking_send(page);
                    Some(())
                })
                .count()
        });

        let mut batch = Vec::with_capacity(100);
        let mut timeout = tokio::time::interval(tokio::time::Duration::from_secs(1));
        let mut total = 0;

        loop {
            tokio::select! {
                biased;
                page = rx.recv() => {
                    if let Some(page) = page {
                        batch.push(page);
                        total += 1;

                        if batch.len() >= 100 {
                            self.commit_batch(batch, &self.staging_index).await?;
                            batch = Vec::with_capacity(100);
                        }
                    } else {
                        break;                    }
                },
                _ = timeout.tick() => {
                    if !batch.is_empty() {
                        self.commit_batch(batch, &self.staging_index).await?;
                        batch = Vec::with_capacity(100);
                    }
                }
            }
        }

        if !batch.is_empty() {
            self.commit_batch(batch, &self.staging_index).await?;
        }

        let _ = producer.await?;

        let delta = start.elapsed()?;
        tracing::info!("\tIndexed {} pages in {:?}", total, delta);

        Ok(())
    }

    async fn swap_indexes(&mut self) -> Result<()> {
        tracing::debug!("Swapping active and staging indexes");

        let _ = remove_dummy_index(&self.active_path);
        let old_active_index = std::mem::replace(
            &mut self.active_index,
            create_dummy_index(&self.active_path)?,
        );
        let event = old_active_index.prepare_for_closing();
        event.wait();

        let _ = remove_dummy_index(&self.staging_path);
        let old_staging_index = std::mem::replace(
            &mut self.staging_index,
            create_dummy_index(&self.staging_path)?,
        );
        let event = old_staging_index.prepare_for_closing();
        event.wait();

        let alpha_is_active = self.active_path.canonicalize()? == self.alpha_path;

        let _ = fs::remove_file(&self.active_path);
        let _ = fs::remove_file(&self.staging_path);

        if alpha_is_active {
            symlink(&self.beta_path, &self.active_path)?;
            symlink(&self.alpha_path, &self.staging_path)?;
        } else {
            symlink(&self.alpha_path, &self.active_path)?;
            symlink(&self.beta_path, &self.staging_path)?;
        }

        let dummy_active_index = std::mem::replace(
            &mut self.active_index,
            create_or_open_index(&self.active_path)?,
        );
        let event = dummy_active_index.prepare_for_closing();
        event.wait();
        remove_dummy_index(&self.active_path)?;

        let dummy_staging_index = std::mem::replace(
            &mut self.staging_index,
            create_or_open_index(&self.staging_path)?,
        );
        let event = dummy_staging_index.prepare_for_closing();
        event.wait();
        remove_dummy_index(&self.staging_path)?;

        tracing::debug!("Index swap completed successfully");
        Ok(())
    }

    async fn clear_staging(&self) -> Result<()> {
        tracing::debug!("Clear out staging");
        let mut wtxn = self.staging_index.write_txn()?;
        let clear = ClearDocuments::new(&mut wtxn, &self.staging_index);
        clear.execute()?;
        wtxn.commit()?;
        Ok(())
    }

    async fn commit_batch(&self, batch: Vec<Page>, index: &Index) -> Result<()> {
        tracing::debug!("Indexing batch of {} pages", batch.len());
        let mut wtxn = index.write_txn()?;

        let config = IndexerConfig::default();
        let indexing_config = IndexDocumentsConfig::default();
        let mut builder = DocumentsBatchBuilder::new(Vec::new());

        for page in batch {
            builder.append_json_object(
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
        }

        let vector = builder.into_inner().unwrap();
        let reader = DocumentsBatchReader::from_reader(Cursor::new(vector))?;

        let (builder, _) =
            IndexDocuments::new(&mut wtxn, index, &config, indexing_config, |_| (), || false)?
                .add_documents(reader)?;

        builder.execute()?;
        wtxn.commit()?;

        Ok(())
    }
}

fn create_or_open_index(path: &Path) -> Result<Index> {
    fs::create_dir_all(path)?;

    let mut options = EnvOpenOptions::new();
    options.map_size(1024 * 1024 * 1024);
    options.max_dbs(1);
    options.max_readers(512);
    let options = options.read_txn_without_tls();
    let options_spare = options.clone();

    let index_result = Index::new(options, path, true);
    let index = if let Ok(i) = index_result {
        i
    } else {
        fs::remove_dir_all(path)?;
        fs::create_dir_all(path)?;
        Index::new(options_spare, path, true)?
    };

    let mut wtxn = index.write_txn()?;
    let config = IndexerConfig::default();
    let mut builder = Settings::new(&mut wtxn, &index, &config);
    builder.set_primary_key("id".into());
    builder.set_searchable_fields(vec!["title".into(), "markdown".into(), "tags".into()]);
    builder.execute(|_| (), || false)?;
    wtxn.commit()?;

    Ok(index)
}

fn create_dummy_index(path: &Path) -> Result<Index> {
    let path = path.with_extension("dummy");
    std::fs::create_dir_all(&path)?;

    let mut options = EnvOpenOptions::new();
    options.map_size(1024 * 1024);
    let options = options.read_txn_without_tls();
    let index = Index::new(options, path, true)?;

    Ok(index)
}

fn remove_dummy_index(path: &Path) -> Result<()> {
    let path = path.with_extension("dummy");
    std::fs::remove_dir_all(&path)?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn symlink(original: &PathBuf, link: &PathBuf) -> std::io::Result<()> {
    std::os::unix::fs::symlink(original, link)
}

#[cfg(target_os = "windows")]
fn symlink(original: &PathBuf, link: &PathBuf) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(original, link)
}

#[derive(Deserialize)]
struct SearchParams {
    q: String,
}

fn render_search_results(query: String, results: Vec<serde_json::Value>) -> Html<String> {
    let mut results_html = String::new();
    for result in &results {
        let result_html = format!(
            r#"
            <article class="search-result">
                <h3>
                    <a href="{}">{}</a>
                </h3>
                <p>{}</p>
            </article>
        "#,
            result.get("url").map(|v| v.to_string()).unwrap_or_default(),
            result
                .get("title")
                .map(|v| v.to_string())
                .unwrap_or_default(),
            result
                .get("_formatted_markdown")
                .map(|v| v.to_string())
                .unwrap_or_default(),
        );
        results_html.push_str(&result_html);
    }

    let html = format!(
        r#"<!DOCTYPE html>
<html>
    <head>
        <meta http-equiv="Content-Type" content="text/html; charset=UTF-8">
        <meta http-equiv="X-UA-Compatible" content="IE=Edge">
        <meta name="viewport" content="width=device-width,initial-scale=1">
        <title>Search results for: {}</title>
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
            <h1>Search results for: {}</h1>
            <p>Found {} results</p>
            <ol class="search-results">{}</ol>
        </main>
    </body>
</html>"#,
        &query,
        ASSET_MANAGER.hashed_route("styles.css").unwrap_or_default(),
        ASSET_MANAGER.hashed_route("script.js").unwrap_or_default(),
        &query,
        results.len(),
        results_html
    );

    Html(html)
}
