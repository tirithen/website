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
    pub fn read(path: &Path) -> Result<Self, PageError> {
        let content = fs::read_to_string(path)?;
        let modified = fs::metadata(path)?.modified()?;

        let (frontmatter, markdown) = Self::split_frontmatter(&content)?;

        let html = Self::render_markdown(&markdown)?;
        let url = Self::path_to_url(path);

        let title = if frontmatter.title.is_some() {
            frontmatter.title
        } else {
            let document = Html::parse_document(&html);
            Self::extract_h1_title(&document)
        };

        Ok(Self {
            id: frontmatter.id.unwrap_or_else(Ulid::new),
            title,
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

    fn extract_h1_title(document: &Html) -> Option<String> {
        let selector = Selector::parse("h1").unwrap();
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
        path.strip_prefix("pages")
            .unwrap_or(path)
            .with_extension("")
            .to_path_buf()
    }
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
        dbg!(&fm, &md);
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
