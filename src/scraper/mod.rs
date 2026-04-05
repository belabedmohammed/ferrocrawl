mod content_cleaner;
mod static_scraper;

pub use content_cleaner::ContentCleaner;
pub use static_scraper::StaticScraper;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ScrapeResult {
    pub url: String,
    pub status_code: u16,
    pub content_type: Option<String>,
    pub markdown: String,
    pub raw_html: Option<String>,
    pub metadata: PageMetadata,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct PageMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub language: Option<String>,
    pub og_image: Option<String>,
    pub canonical_url: Option<String>,
    pub word_count: usize,
}
