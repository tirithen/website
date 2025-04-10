use pulldown_cmark::{Parser, html};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;
use time::OffsetDateTime;
use ulid::Ulid;
use walkdir::WalkDir;
use xxhash_rust::xxh3::xxh3_128;

use crate::config::load_config;

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Frontmatter {
    pub id: Option<Ulid>,
    pub title: Option<String>,
    pub tags: Option<HashSet<String>>,
}

#[derive(Debug)]
pub struct Page {
    pub id: Ulid,
    pub title: Option<String>,
    pub modified: OffsetDateTime,
    pub url: PathBuf,
    pub tags: HashSet<String>,
    pub markdown: String,
    pub html: String,
}

#[derive(Error, Debug)]
pub enum PageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Toml error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("YAML error: {0}")]
    YAMLDeserialize(#[from] serde_yaml::Error),
}

impl Page {
    pub fn read(path: impl Into<PathBuf>) -> Result<Self, PageError> {
        let path = Self::get_full_path(path)?;
        let content = fs::read_to_string(&path)?;
        let modified = fs::metadata(&path)?.modified()?;

        let (frontmatter, markdown) = Self::split_frontmatter(&content)?;

        let html = Self::render_markdown(&markdown)?;
        let url = Self::path_to_url(&path);

        let title = if frontmatter.title.is_some() {
            frontmatter.title
        } else {
            let document = Html::parse_document(&html);
            Self::extract_header_title(&document)
        };

        Ok(Self {
            title: title.clone(),
            id: frontmatter
                .id
                .unwrap_or_else(|| ulid_from_string(&title.unwrap_or_default())),
            modified: OffsetDateTime::from(modified),
            url,
            tags: frontmatter.tags.unwrap_or_default(),
            markdown,
            html,
        })
    }

    pub fn write(&self, base_path: &Path) -> Result<(), PageError> {
        let path = base_path.join(&self.url).with_extension("md");
        let frontmatter = toml::to_string(&Frontmatter {
            id: Some(Ulid::new()),
            title: self.title.clone(),
            tags: Some(self.tags.clone()),
        })?;

        let content = format!(
            "---\n{}\n---\n{}",
            ammonia::clean(&frontmatter),
            ammonia::clean(&self.markdown)
        );
        fs::write(path, content)?;
        Ok(())
    }

    pub fn all() -> impl Iterator<Item = Page> {
        let pages_root = load_config().pages_path();
        WalkDir::new(pages_root)
            .follow_links(true)
            .into_iter()
            .filter_map(|entry| {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => return None,
                };

                if !entry.file_type().is_file() || entry.path().extension()? != "md" {
                    return None;
                }

                match Page::read(entry.path()) {
                    Ok(page) => Some(page),
                    Err(_) => None,
                }
            })
    }

    fn get_full_path(url_path: impl Into<PathBuf>) -> Result<PathBuf, PageError> {
        let path: PathBuf = url_path.into();
        let mut path = path.to_string_lossy().to_string();

        if path.is_empty() {
            path = "/".into();
        }

        if path.ends_with("/") {
            path.push_str("index.md");
        } else if !path.ends_with(".md") {
            path.push_str(".md");
        }
        path = path
            .strip_prefix("/")
            .map(|p| p.into())
            .unwrap_or(path.clone());

        let config = load_config();
        let pages_root = config.pages_path();
        let file_path = pages_root.join(&path).canonicalize()?;

        if !file_path.starts_with(&pages_root) {
            return Err(PageError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Path is outside data root",
            )));
        }

        Ok(file_path)
    }

    fn extract_header_title(document: &Html) -> Option<String> {
        let selector = Selector::parse("h1,h2,h3,h4,h5,h6,p").unwrap();
        document
            .select(&selector)
            .next()
            .map(|h1| h1.text().collect::<String>())
            .map(|s| s.trim().to_string())
    }

    fn split_frontmatter(content: &str) -> Result<(Frontmatter, String), PageError> {
        let mut lines = content.lines();
        if lines.next() != Some("---") {
            return Ok((Frontmatter::default(), content.to_string()));
        }

        let mut frontmatter = String::new();
        for line in lines.by_ref() {
            if line == "---" {
                break;
            }
            frontmatter.push_str(line);
            frontmatter.push('\n');
        }

        let markdown = lines
            .fold(String::new(), |mut result, value| {
                result.push_str(value);
                result.push('\n');
                result
            })
            .trim()
            .to_string();

        let frontmatter: Frontmatter = serde_yaml::from_str(&frontmatter)?;

        Ok((frontmatter, markdown))
    }

    fn render_markdown(markdown: &str) -> Result<String, PageError> {
        let parser = Parser::new(markdown);
        let mut html = String::new();
        html::push_html(&mut html, parser);
        let html = ammonia::clean(&html);
        Ok(html.trim().to_string())
    }

    fn path_to_url(path: &Path) -> PathBuf {
        let config = load_config();
        path.strip_prefix(config.pages_path())
            .unwrap_or(path)
            .with_extension("")
            .to_path_buf()
    }
}

fn ulid_from_string(input: &str) -> Ulid {
    let hash = xxh3_128(input.as_bytes());
    Ulid::from_parts(0, hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frontmatter_parsing() {
        let content = r#"---
title: "Test Page"
tags: ["rust", "axum"]
---
# Content

Some other text
"#;

        let (fm, md) = Page::split_frontmatter(content).unwrap();
        assert_eq!(fm.title, Some("Test Page".into()));
        assert_eq!(
            fm.tags.unwrap(),
            HashSet::from(["rust".into(), "axum".into()])
        );
        assert_eq!(md.trim(), "# Content\n\nSome other text");
    }

    #[test]
    fn test_link_rendering() {
        let md = "[About Page](/about-page)";
        let html = Page::render_markdown(md).unwrap();
        assert_eq!(html, r#"<p><a href="/about-page">About Page</a></p>"#);
    }
}
