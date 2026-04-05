use std::time::{Duration, Instant};

use moka::future::Cache;
use reqwest::Client;
use url::Url;

use super::{ContentCleaner, ScrapeResult};
use crate::config::Config;
use crate::error::AppError;

pub struct StaticScraper {
    client: Client,
    cache: Cache<String, ScrapeResult>,
    max_body_size: usize,
}

impl StaticScraper {
    pub fn new(config: &Config) -> Result<Self, AppError> {
        let client = Client::builder()
            .user_agent(&config.scraper.user_agent)
            .timeout(config.scraper.request_timeout)
            .connect_timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::limited(5))
            .gzip(true)
            .brotli(true)
            .deflate(true)
            .build()
            .map_err(|e| AppError::Internal(format!("Failed to create HTTP client: {e}")))?;

        let cache = Cache::builder()
            .max_capacity(config.cache.max_capacity)
            .time_to_live(Duration::from_secs(config.cache.ttl_seconds))
            .build();

        Ok(Self {
            client,
            cache,
            max_body_size: config.scraper.max_body_size,
        })
    }

    pub async fn scrape(
        &self,
        url: &str,
        include_raw_html: bool,
    ) -> Result<ScrapeResult, AppError> {
        // Validate URL
        let parsed = Url::parse(url).map_err(|e| AppError::InvalidUrl(e.to_string()))?;

        // Only allow http/https
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(AppError::InvalidUrl(
                "Only http and https URLs are supported".into(),
            ));
        }

        // Block private IPs (SSRF protection)
        if let Some(host) = parsed.host_str() {
            if Self::is_private_host(host) {
                return Err(AppError::InvalidUrl(
                    "Private/internal URLs are not allowed".into(),
                ));
            }
        }

        // Check cache
        let cache_key = format!("{url}:{include_raw_html}");
        if let Some(cached) = self.cache.get(&cache_key).await {
            tracing::debug!(url, "cache.hit");
            return Ok(cached);
        }

        let start = Instant::now();

        // Fetch
        let response = self
            .client
            .get(url)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .header("Accept-Language", "fr-FR,fr;q=0.9,en-US;q=0.8,en;q=0.7")
            .send()
            .await?;

        let status_code = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // Check status
        if status_code == 403 || status_code == 429 {
            return Err(AppError::Blocked);
        }

        // Check content length before downloading body
        if let Some(len) = response.content_length() {
            if len as usize > self.max_body_size {
                return Err(AppError::ContentTooLarge(len as usize));
            }
        }

        let raw_html = response
            .text()
            .await
            .map_err(|e| AppError::FetchError(format!("Failed to read body: {e}")))?;

        if raw_html.len() > self.max_body_size {
            return Err(AppError::ContentTooLarge(raw_html.len()));
        }

        // Extract metadata from raw HTML
        let metadata = ContentCleaner::extract_metadata(&raw_html);

        // Clean HTML then convert to markdown
        let cleaned_html = ContentCleaner::clean_html(&raw_html);
        let markdown = htmd::convert(&cleaned_html).unwrap_or_else(|_| cleaned_html.clone());
        let markdown = Self::normalize_whitespace(&markdown);

        let elapsed_ms = start.elapsed().as_millis() as u64;

        let result = ScrapeResult {
            url: url.to_string(),
            status_code,
            content_type,
            markdown,
            raw_html: if include_raw_html {
                Some(raw_html)
            } else {
                None
            },
            metadata,
            elapsed_ms,
        };

        self.cache.insert(cache_key, result.clone()).await;

        Ok(result)
    }

    fn is_private_host(host: &str) -> bool {
        if matches!(host, "localhost" | "127.0.0.1" | "0.0.0.0" | "::1") {
            return true;
        }
        if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
            return ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_unspecified();
        }
        if host.ends_with(".local") || host.ends_with(".internal") {
            return true;
        }
        false
    }

    fn normalize_whitespace(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut blank_count = 0u32;

        for line in text.lines() {
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                blank_count += 1;
            } else {
                // Insert at most one blank line between content lines
                if !result.is_empty() && blank_count > 0 {
                    result.push('\n');
                }
                result.push_str(trimmed);
                result.push('\n');
                blank_count = 0;
            }
        }

        result.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- SSRF protection ---

    #[test]
    fn is_private_host_localhost() {
        assert!(StaticScraper::is_private_host("localhost"));
        assert!(StaticScraper::is_private_host("127.0.0.1"));
        assert!(StaticScraper::is_private_host("0.0.0.0"));
        assert!(StaticScraper::is_private_host("::1"));
    }

    #[test]
    fn is_private_host_rfc1918() {
        assert!(StaticScraper::is_private_host("10.0.0.1"));
        assert!(StaticScraper::is_private_host("10.255.255.255"));
        assert!(StaticScraper::is_private_host("172.16.0.1"));
        assert!(StaticScraper::is_private_host("192.168.0.1"));
        assert!(StaticScraper::is_private_host("192.168.1.100"));
    }

    #[test]
    fn is_private_host_link_local() {
        assert!(StaticScraper::is_private_host("169.254.0.1"));
        assert!(StaticScraper::is_private_host("169.254.169.254")); // AWS metadata
    }

    #[test]
    fn is_private_host_local_domains() {
        assert!(StaticScraper::is_private_host("myservice.local"));
        assert!(StaticScraper::is_private_host("db.internal"));
    }

    #[test]
    fn is_private_host_public_ips_allowed() {
        assert!(!StaticScraper::is_private_host("8.8.8.8"));
        assert!(!StaticScraper::is_private_host("1.1.1.1"));
        assert!(!StaticScraper::is_private_host("93.184.216.34")); // example.com
    }

    #[test]
    fn is_private_host_public_domains_allowed() {
        assert!(!StaticScraper::is_private_host("example.com"));
        assert!(!StaticScraper::is_private_host("google.com"));
        assert!(!StaticScraper::is_private_host("api.example.com"));
    }

    // --- Whitespace normalization ---

    #[test]
    fn normalize_whitespace_collapses_blank_lines() {
        let input = "line1\n\n\n\n\nline2";
        let result = StaticScraper::normalize_whitespace(input);
        assert_eq!(result, "line1\n\nline2");
    }

    #[test]
    fn normalize_whitespace_trims_trailing() {
        let input = "hello   \nworld   ";
        let result = StaticScraper::normalize_whitespace(input);
        assert_eq!(result, "hello\nworld");
    }

    #[test]
    fn normalize_whitespace_empty_input() {
        let result = StaticScraper::normalize_whitespace("");
        assert_eq!(result, "");
    }

    #[test]
    fn normalize_whitespace_single_line() {
        let result = StaticScraper::normalize_whitespace("hello world");
        assert_eq!(result, "hello world");
    }

    #[test]
    fn normalize_whitespace_preserves_single_blank() {
        let input = "a\n\nb";
        let result = StaticScraper::normalize_whitespace(input);
        assert_eq!(result, "a\n\nb");
    }

    // --- URL validation (via scrape method) ---

    fn make_scraper() -> StaticScraper {
        let config = Config {
            server: crate::config::ServerConfig {
                host: "0.0.0.0".into(),
                port: 3400,
            },
            scraper: crate::config::ScraperConfig {
                request_timeout: Duration::from_secs(5),
                max_body_size: 1024 * 1024,
                max_concurrent: 10,
                user_agent: "test".into(),
            },
            cache: crate::config::CacheConfig {
                ttl_seconds: 60,
                max_capacity: 10,
            },
            anthropic: crate::config::AnthropicConfig {
                api_key: None,
                model: "test".into(),
            },
            auth: crate::config::AuthConfig { api_keys: vec![] },
        };
        StaticScraper::new(&config).unwrap()
    }

    #[tokio::test]
    async fn scrape_rejects_ftp_scheme() {
        let scraper = make_scraper();
        let result = scraper.scrape("ftp://files.example.com/data", false).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Only http and https"));
    }

    #[tokio::test]
    async fn scrape_rejects_file_scheme() {
        let scraper = make_scraper();
        let result = scraper.scrape("file:///etc/passwd", false).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn scrape_rejects_localhost() {
        let scraper = make_scraper();
        let result = scraper.scrape("http://localhost:8080/admin", false).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Private"));
    }

    #[tokio::test]
    async fn scrape_rejects_private_ip() {
        let scraper = make_scraper();
        let result = scraper.scrape("http://192.168.1.1/admin", false).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Private"));
    }

    #[tokio::test]
    async fn scrape_rejects_aws_metadata() {
        let scraper = make_scraper();
        let result = scraper
            .scrape("http://169.254.169.254/latest/meta-data/", false)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn scrape_rejects_invalid_url() {
        let scraper = make_scraper();
        let result = scraper.scrape("not-a-url", false).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn scrape_rejects_empty_url() {
        let scraper = make_scraper();
        let result = scraper.scrape("", false).await;
        assert!(result.is_err());
    }
}
