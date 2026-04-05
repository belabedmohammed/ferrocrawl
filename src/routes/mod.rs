mod health;
mod scrape;
mod extract;

pub use health::health_check;
pub use scrape::scrape_url;
pub use extract::extract_data;
