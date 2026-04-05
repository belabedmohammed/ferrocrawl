use std::env;
use std::time::Duration;

use crate::error::AppError;

#[derive(Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub scraper: ScraperConfig,
    pub cache: CacheConfig,
    pub anthropic: AnthropicConfig,
    pub auth: AuthConfig,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("server", &self.server)
            .field("scraper", &self.scraper)
            .field("cache", &self.cache)
            .field("anthropic", &self.anthropic)
            .field("auth", &self.auth)
            .finish()
    }
}

#[derive(Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl std::fmt::Debug for ServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .finish()
    }
}

#[derive(Clone)]
pub struct ScraperConfig {
    pub request_timeout: Duration,
    pub max_body_size: usize,
    pub max_concurrent: usize,
    pub user_agent: String,
}

impl std::fmt::Debug for ScraperConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScraperConfig")
            .field("request_timeout", &self.request_timeout)
            .field("max_body_size", &self.max_body_size)
            .field("max_concurrent", &self.max_concurrent)
            .finish()
    }
}

#[derive(Clone)]
pub struct CacheConfig {
    pub ttl_seconds: u64,
    pub max_capacity: u64,
}

impl std::fmt::Debug for CacheConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheConfig")
            .field("ttl_seconds", &self.ttl_seconds)
            .field("max_capacity", &self.max_capacity)
            .finish()
    }
}

#[derive(Clone)]
pub struct AnthropicConfig {
    pub api_key: Option<String>,
    pub model: String,
}

impl std::fmt::Debug for AnthropicConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnthropicConfig")
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field("model", &self.model)
            .finish()
    }
}

#[derive(Clone)]
pub struct AuthConfig {
    pub api_keys: Vec<String>,
}

impl std::fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthConfig")
            .field("api_keys", &format!("[{} keys]", self.api_keys.len()))
            .finish()
    }
}

impl AuthConfig {
    pub fn enabled(&self) -> bool {
        !self.api_keys.is_empty()
    }
}

impl Config {
    pub fn from_env() -> Result<Self, AppError> {
        Ok(Self {
            server: ServerConfig {
                host: env::var("FERROCRAWL_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
                port: env::var("FERROCRAWL_PORT")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(3400),
            },
            scraper: ScraperConfig {
                request_timeout: Duration::from_secs(
                    env::var("FERROCRAWL_TIMEOUT_SECS")
                        .ok()
                        .and_then(|t| t.parse().ok())
                        .unwrap_or(30),
                ),
                max_body_size: env::var("FERROCRAWL_MAX_BODY_SIZE")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10 * 1024 * 1024), // 10MB
                max_concurrent: env::var("FERROCRAWL_MAX_CONCURRENT")
                    .ok()
                    .and_then(|c| c.parse().ok())
                    .unwrap_or(50),
                user_agent: env::var("FERROCRAWL_USER_AGENT").unwrap_or_else(|_| {
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36".into()
                }),
            },
            cache: CacheConfig {
                ttl_seconds: env::var("FERROCRAWL_CACHE_TTL")
                    .ok()
                    .and_then(|t| t.parse().ok())
                    .unwrap_or(300),
                max_capacity: env::var("FERROCRAWL_CACHE_MAX")
                    .ok()
                    .and_then(|c| c.parse().ok())
                    .unwrap_or(1000),
            },
            anthropic: AnthropicConfig {
                api_key: env::var("ANTHROPIC_API_KEY").ok(),
                model: env::var("ANTHROPIC_MODEL")
                    .unwrap_or_else(|_| "claude-sonnet-4-20250514".into()),
            },
            auth: AuthConfig {
                api_keys: env::var("FERROCRAWL_API_KEYS")
                    .ok()
                    .map(|keys| {
                        keys.split(',')
                            .map(|k| k.trim().to_string())
                            .filter(|k| !k.is_empty() && k != "none")
                            .collect()
                    })
                    .unwrap_or_default(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_enabled_with_keys() {
        let auth = AuthConfig {
            api_keys: vec!["key1".into(), "key2".into()],
        };
        assert!(auth.enabled());
    }

    #[test]
    fn auth_disabled_without_keys() {
        let auth = AuthConfig {
            api_keys: vec![],
        };
        assert!(!auth.enabled());
    }

    #[test]
    fn config_debug_redacts_api_key() {
        let config = AnthropicConfig {
            api_key: Some("sk-secret-key-12345".into()),
            model: "claude-test".into(),
        };
        let debug_str = format!("{:?}", config);
        assert!(!debug_str.contains("sk-secret-key-12345"));
        assert!(debug_str.contains("[REDACTED]"));
        assert!(debug_str.contains("claude-test"));
    }

    #[test]
    fn config_debug_redacts_auth_keys() {
        let auth = AuthConfig {
            api_keys: vec!["secret1".into(), "secret2".into()],
        };
        let debug_str = format!("{:?}", auth);
        assert!(!debug_str.contains("secret1"));
        assert!(!debug_str.contains("secret2"));
        assert!(debug_str.contains("2 keys"));
    }

    #[test]
    fn config_from_env_defaults() {
        // Clear any env vars that might be set
        env::remove_var("FERROCRAWL_HOST");
        env::remove_var("FERROCRAWL_PORT");
        env::remove_var("FERROCRAWL_API_KEYS");

        let config = Config::from_env().unwrap();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 3400);
        assert_eq!(config.cache.ttl_seconds, 300);
        assert_eq!(config.cache.max_capacity, 1000);
        assert_eq!(config.scraper.max_body_size, 10 * 1024 * 1024);
        assert_eq!(config.scraper.max_concurrent, 50);
        assert!(!config.auth.enabled());
    }

    #[test]
    fn server_config_debug_shows_fields() {
        let server = ServerConfig {
            host: "0.0.0.0".into(),
            port: 3400,
        };
        let debug_str = format!("{:?}", server);
        assert!(debug_str.contains("0.0.0.0"));
        assert!(debug_str.contains("3400"));
    }
}
