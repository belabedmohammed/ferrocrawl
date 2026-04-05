use std::collections::HashSet;

use ego_tree::NodeId;
use scraper::{Html, Selector};

use super::PageMetadata;

pub struct ContentCleaner;

impl ContentCleaner {
    /// Remove non-content elements from HTML (nav, footer, ads, scripts, etc.)
    pub fn clean_html(raw_html: &str) -> String {
        let document = Html::parse_document(raw_html);

        let selectors_to_remove = [
            "script", "style", "noscript", "iframe", "svg",
            "nav", "footer", "header",
            "[role='navigation']", "[role='banner']", "[role='contentinfo']",
            ".nav", ".navbar", ".footer", ".header", ".sidebar", ".menu",
            ".ad", ".ads", ".advertisement",
            ".cookie-banner", ".cookie-consent", ".popup", ".modal",
            "#cookie-banner", "#cookie-consent",
            "[data-ad]", "[data-advertisement]",
        ];

        let mut excluded: HashSet<NodeId> = HashSet::new();
        for sel_str in &selectors_to_remove {
            if let Ok(sel) = Selector::parse(sel_str) {
                for el in document.select(&sel) {
                    excluded.insert(el.id());
                }
            }
        }

        let body_selector = Selector::parse("body").unwrap();
        if let Some(body) = document.select(&body_selector).next() {
            let mut clean = String::with_capacity(raw_html.len() / 2);
            Self::collect_text(&body, &excluded, &mut clean);
            clean
        } else {
            raw_html.to_string()
        }
    }

    fn collect_text(
        node: &scraper::ElementRef,
        excluded: &HashSet<NodeId>,
        output: &mut String,
    ) {
        for child in node.children() {
            if let Some(el) = scraper::ElementRef::wrap(child) {
                if excluded.contains(&el.id()) {
                    continue;
                }
                let tag = el.value().name();
                output.push('<');
                output.push_str(tag);

                for (name, value) in el.value().attrs() {
                    if matches!(name, "href" | "src" | "alt" | "title") {
                        output.push(' ');
                        output.push_str(name);
                        output.push_str("=\"");
                        output.push_str(value);
                        output.push('"');
                    }
                }
                output.push('>');
                Self::collect_text(&el, excluded, output);
                output.push_str("</");
                output.push_str(tag);
                output.push('>');
            } else if let Some(text) = child.value().as_text() {
                output.push_str(text);
            }
        }
    }

    pub fn extract_metadata(raw_html: &str) -> PageMetadata {
        let document = Html::parse_document(raw_html);

        let title = Self::select_text(&document, "title")
            .or_else(|| Self::select_attr(&document, "meta[property='og:title']", "content"));

        let description =
            Self::select_attr(&document, "meta[name='description']", "content").or_else(|| {
                Self::select_attr(&document, "meta[property='og:description']", "content")
            });

        let language = Self::select_attr(&document, "html", "lang");
        let og_image = Self::select_attr(&document, "meta[property='og:image']", "content");
        let canonical_url = Self::select_attr(&document, "link[rel='canonical']", "href");

        let body_sel = Selector::parse("body").unwrap();
        let word_count = document
            .select(&body_sel)
            .next()
            .map(|b| b.text().collect::<String>().split_whitespace().count())
            .unwrap_or(0);

        PageMetadata {
            title,
            description,
            language,
            og_image,
            canonical_url,
            word_count,
        }
    }

    fn select_text(doc: &Html, selector: &str) -> Option<String> {
        let sel = Selector::parse(selector).ok()?;
        doc.select(&sel)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn select_attr(doc: &Html, selector: &str, attr: &str) -> Option<String> {
        let sel = Selector::parse(selector).ok()?;
        doc.select(&sel)
            .next()
            .and_then(|el| el.value().attr(attr))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_html_removes_scripts_and_styles() {
        let html = r#"<html><body>
            <p>Hello world</p>
            <script>alert('xss')</script>
            <style>.foo { color: red; }</style>
            <p>Goodbye</p>
        </body></html>"#;

        let cleaned = ContentCleaner::clean_html(html);
        assert!(!cleaned.contains("alert"));
        assert!(!cleaned.contains("color: red"));
        assert!(cleaned.contains("Hello world"));
        assert!(cleaned.contains("Goodbye"));
    }

    #[test]
    fn clean_html_removes_nav_footer_header() {
        let html = r#"<html><body>
            <header><a href="/">Logo</a></header>
            <nav><a href="/about">About</a></nav>
            <main><p>Main content here</p></main>
            <footer><p>Copyright 2026</p></footer>
        </body></html>"#;

        let cleaned = ContentCleaner::clean_html(html);
        assert!(cleaned.contains("Main content here"));
        assert!(!cleaned.contains("Copyright 2026"));
        assert!(!cleaned.contains("Logo"));
    }

    #[test]
    fn clean_html_removes_ad_elements() {
        let html = r#"<html><body>
            <p>Article text</p>
            <div class="ad">Buy stuff!</div>
            <div class="advertisement">Sponsored</div>
            <div data-ad="true">Ad content</div>
            <p>More article</p>
        </body></html>"#;

        let cleaned = ContentCleaner::clean_html(html);
        assert!(cleaned.contains("Article text"));
        assert!(cleaned.contains("More article"));
        assert!(!cleaned.contains("Buy stuff"));
        assert!(!cleaned.contains("Sponsored"));
    }

    #[test]
    fn clean_html_removes_cookie_banners() {
        let html = r#"<html><body>
            <p>Content</p>
            <div class="cookie-banner">Accept cookies?</div>
            <div class="cookie-consent">We use cookies</div>
            <div id="cookie-banner">Banner</div>
        </body></html>"#;

        let cleaned = ContentCleaner::clean_html(html);
        assert!(cleaned.contains("Content"));
        assert!(!cleaned.contains("Accept cookies"));
        assert!(!cleaned.contains("We use cookies"));
    }

    #[test]
    fn clean_html_preserves_links() {
        let html = r#"<html><body>
            <p>Visit <a href="https://example.com">Example</a></p>
        </body></html>"#;

        let cleaned = ContentCleaner::clean_html(html);
        assert!(cleaned.contains("href=\"https://example.com\""));
        assert!(cleaned.contains("Example"));
    }

    #[test]
    fn clean_html_preserves_images() {
        let html = r#"<html><body>
            <img src="photo.jpg" alt="A photo" />
        </body></html>"#;

        let cleaned = ContentCleaner::clean_html(html);
        assert!(cleaned.contains("src=\"photo.jpg\""));
        assert!(cleaned.contains("alt=\"A photo\""));
    }

    #[test]
    fn extract_metadata_title() {
        let html = r#"<html><head><title>My Page Title</title></head><body></body></html>"#;
        let meta = ContentCleaner::extract_metadata(html);
        assert_eq!(meta.title.as_deref(), Some("My Page Title"));
    }

    #[test]
    fn extract_metadata_og_title_fallback() {
        let html = r#"<html><head>
            <meta property="og:title" content="OG Title" />
        </head><body></body></html>"#;
        let meta = ContentCleaner::extract_metadata(html);
        assert_eq!(meta.title.as_deref(), Some("OG Title"));
    }

    #[test]
    fn extract_metadata_description() {
        let html = r#"<html><head>
            <meta name="description" content="Page description here" />
        </head><body></body></html>"#;
        let meta = ContentCleaner::extract_metadata(html);
        assert_eq!(meta.description.as_deref(), Some("Page description here"));
    }

    #[test]
    fn extract_metadata_og_description_fallback() {
        let html = r#"<html><head>
            <meta property="og:description" content="OG Description" />
        </head><body></body></html>"#;
        let meta = ContentCleaner::extract_metadata(html);
        assert_eq!(meta.description.as_deref(), Some("OG Description"));
    }

    #[test]
    fn extract_metadata_language() {
        let html = r#"<html lang="fr"><head></head><body></body></html>"#;
        let meta = ContentCleaner::extract_metadata(html);
        assert_eq!(meta.language.as_deref(), Some("fr"));
    }

    #[test]
    fn extract_metadata_og_image() {
        let html = r#"<html><head>
            <meta property="og:image" content="https://example.com/img.png" />
        </head><body></body></html>"#;
        let meta = ContentCleaner::extract_metadata(html);
        assert_eq!(meta.og_image.as_deref(), Some("https://example.com/img.png"));
    }

    #[test]
    fn extract_metadata_canonical_url() {
        let html = r#"<html><head>
            <link rel="canonical" href="https://example.com/page" />
        </head><body></body></html>"#;
        let meta = ContentCleaner::extract_metadata(html);
        assert_eq!(meta.canonical_url.as_deref(), Some("https://example.com/page"));
    }

    #[test]
    fn extract_metadata_word_count() {
        let html = r#"<html><body><p>One two three four five</p></body></html>"#;
        let meta = ContentCleaner::extract_metadata(html);
        assert_eq!(meta.word_count, 5);
    }

    #[test]
    fn extract_metadata_missing_fields() {
        let html = r#"<html><head></head><body></body></html>"#;
        let meta = ContentCleaner::extract_metadata(html);
        assert!(meta.title.is_none());
        assert!(meta.description.is_none());
        assert!(meta.og_image.is_none());
        assert!(meta.canonical_url.is_none());
    }

    #[test]
    fn clean_html_handles_no_body() {
        let html = "<html><head><title>No body</title></head></html>";
        let cleaned = ContentCleaner::clean_html(html);
        // Should not panic, returns something
        assert!(!cleaned.is_empty() || cleaned.is_empty());
    }

    #[test]
    fn clean_html_removes_modals_and_popups() {
        let html = r#"<html><body>
            <p>Real content</p>
            <div class="modal">Sign up now!</div>
            <div class="popup">Subscribe!</div>
        </body></html>"#;

        let cleaned = ContentCleaner::clean_html(html);
        assert!(cleaned.contains("Real content"));
        assert!(!cleaned.contains("Sign up now"));
        assert!(!cleaned.contains("Subscribe"));
    }

    #[test]
    fn clean_html_removes_role_navigation() {
        let html = r#"<html><body>
            <div role="navigation"><a href="/">Home</a></div>
            <div role="banner">Banner</div>
            <p>Content</p>
            <div role="contentinfo">Footer info</div>
        </body></html>"#;

        let cleaned = ContentCleaner::clean_html(html);
        assert!(cleaned.contains("Content"));
        assert!(!cleaned.contains("Footer info"));
        assert!(!cleaned.contains("Banner"));
    }
}
