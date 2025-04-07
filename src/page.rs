use pulldown_cmark::{Options, Parser, html};
use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};
use thiserror::Error;
use time::OffsetDateTime;
use ulid::Ulid;

#[derive(Default, Debug, Serialize, Deserialize)]
struct Frontmatter {
    title: String,
    tags: Option<HashSet<String>>,
}

#[derive(Debug)]
pub struct Page {
    pub id: Ulid,
    pub title: String,
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
    #[error("Markdown render error")]
    Render,
}

impl Page {
    pub fn read(path: &Path) -> Result<Self, PageError> {
        let content = fs::read_to_string(path)?;
        let modified = fs::metadata(path)?.modified()?;

        let (frontmatter, markdown) = Self::split_frontmatter(&content)?;

        let html = Self::render_markdown(&markdown)?;
        let url = Self::path_to_url(path);

        Ok(Self {
            id: Ulid::new(),
            title: frontmatter.title,
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
            title: self.title.clone(),
            tags: Some(self.tags.clone()),
        })?;

        let content = format!("---\n{}\n---\n{}", frontmatter, self.markdown);
        fs::write(path, content)?;
        Ok(())
    }

    fn split_frontmatter(content: &str) -> Result<(Frontmatter, String), PageError> {
        let mut lines = content.lines();
        if lines.next()? != "---" {
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

        let frontmatter: Frontmatter = serde_yaml::from_str(&frontmatter)?;

        Ok((frontmatter, lines))
    }

    fn render_markdown(markdown: &str) -> Result<String, PageError> {
        let parser = Parser::new(markdown);
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);

        let re = Regex::new(r"\[\[([^\]\]]+)\]\]").unwrap();
        let html_with_links = re.replace_all(&html_output, |caps: &Captures| {
            let title = &caps[1];
            let slug = title.replace(' ', "-").to_lowercase();
            format!(r#"<a href="/{}">{}</a>"#, slug, title)
        });

        Ok(html_with_links.to_string())
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
tags = ["rust", "axum"]
---
# Content"#;

        let (fm, md) = Page::split_frontmatter(content).unwrap();
        assert_eq!(fm.title, "Test Page");
        assert_eq!(
            fm.tags.unwrap(),
            HashSet::from(["rust".into(), "axum".into()])
        );
        assert_eq!(md.trim(), "# Content");
    }

    #[test]
    fn test_link_rendering() {
        let md = "[[About Page]]";
        let html = Page::render_markdown(md).unwrap();
        assert_eq!(html, r#"<p><a href="/about-page">About Page</a></p>"#);
    }
}
