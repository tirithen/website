use std::{io::Cursor, path::Path};

use anyhow::Result;
use axum::{Router, extract::Query, response::Html, routing::get};
use heed::EnvOpenOptions;
use milli::{
    Index, Search, SearchResult,
    documents::{DocumentsBatchBuilder, DocumentsBatchReader},
    update::{IndexDocuments, IndexDocumentsConfig, IndexerConfig, Settings},
};
use serde::Deserialize;
use serde_json::Value;

use crate::page::Page;

#[derive(Deserialize)]
struct SearchParams {
    q: String,
}

#[derive(Clone)]
pub struct SearchIndex {
    index: Index,
}

impl SearchIndex {
    pub fn new(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path)?;
        let mut options = EnvOpenOptions::new();
        options.map_size(100 * 1024 * 1024);
        options.max_dbs(1);
        let options = options.read_txn_without_tls();
        let index = Index::new(options, path, true)?;

        let mut wtxn = index.write_txn()?;
        let config = IndexerConfig::default();
        let mut builder = Settings::new(&mut wtxn, &index, &config);
        builder.set_searchable_fields(vec!["markdown".into()]);
        builder.execute(|_| (), || false)?;
        wtxn.commit().unwrap();

        Ok(Self { index })
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
        let rtxn = self.index.read_txn()?;

        let mut search = Search::new(&rtxn, &self.index);
        search.query(query);
        search.limit(10);

        let results: SearchResult = search.execute()?;
        let documents = self.index.documents(&rtxn, results.documents_ids)?;
        let fields_map = self.index.fields_ids_map(&rtxn)?;

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
        let mut wtxn = self.index.write_txn()?;
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
        let builder = IndexDocuments::new(
            &mut wtxn,
            &self.index,
            &config,
            indexing_config,
            |_| (),
            || false,
        )?;

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
