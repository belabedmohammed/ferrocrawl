use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::AnthropicConfig;
use crate::error::AppError;

pub struct LlmExtractor {
    client: Client,
    api_key: Option<String>,
    model: String,
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
    system: String,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

impl LlmExtractor {
    pub fn new(config: &AnthropicConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create LLM HTTP client");

        Self {
            client,
            api_key: config.api_key.clone(),
            model: config.model.clone(),
        }
    }

    pub async fn extract(
        &self,
        markdown: &str,
        schema: Option<&Value>,
        prompt: Option<&str>,
    ) -> Result<Value, AppError> {
        let api_key = self
            .api_key
            .as_ref()
            .ok_or(AppError::LlmNotConfigured)?;

        let system_prompt = Self::build_system_prompt(schema, prompt);

        // Truncate content if too large (keep ~100k chars for Claude)
        let content = if markdown.len() > 100_000 {
            &markdown[..100_000]
        } else {
            markdown
        };

        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            system: system_prompt,
            messages: vec![Message {
                role: "user".into(),
                content: format!("Extract the requested data from this content:\n\n{content}"),
            }],
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| AppError::ExtractionError(format!("LLM request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::ExtractionError(format!(
                "LLM API returned {status}: {body}"
            )));
        }

        let api_response: AnthropicResponse = response
            .json()
            .await
            .map_err(|e| AppError::ExtractionError(format!("Failed to parse LLM response: {e}")))?;

        let text = api_response
            .content
            .first()
            .and_then(|block| block.text.as_ref())
            .ok_or_else(|| AppError::ExtractionError("Empty LLM response".into()))?;

        let json_str = Self::extract_json_block(text);
        serde_json::from_str(json_str)
            .map_err(|e| AppError::ExtractionError(format!("LLM returned invalid JSON: {e}")))
    }

    fn build_system_prompt(schema: Option<&Value>, prompt: Option<&str>) -> String {
        let mut sys = String::from(
            "You are a precise data extraction engine. Extract structured data from the provided web page content.\n\n\
             Rules:\n\
             - Return ONLY valid JSON, no markdown formatting, no explanation\n\
             - If a field cannot be found, use null\n\
             - Be exact: copy values as they appear, don't paraphrase\n\
             - For arrays, include all matching items found\n"
        );

        if let Some(schema) = schema {
            sys.push_str("\nExtract data matching this JSON schema:\n");
            sys.push_str(&serde_json::to_string_pretty(schema).unwrap_or_default());
            sys.push('\n');
        }

        if let Some(prompt) = prompt {
            sys.push_str("\nAdditional instructions: ");
            sys.push_str(prompt);
            sys.push('\n');
        }

        sys
    }

    fn extract_json_block(text: &str) -> &str {
        let trimmed = text.trim();

        if let Some(start) = trimmed.find("```json") {
            let after_marker = &trimmed[start + 7..];
            if let Some(end) = after_marker.find("```") {
                return after_marker[..end].trim();
            }
        }

        if let Some(start) = trimmed.find("```") {
            let after_marker = &trimmed[start + 3..];
            if let Some(end) = after_marker.find("```") {
                return after_marker[..end].trim();
            }
        }

        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_block_plain_json() {
        let input = r#"{"name": "test"}"#;
        assert_eq!(LlmExtractor::extract_json_block(input), r#"{"name": "test"}"#);
    }

    #[test]
    fn extract_json_block_with_json_fence() {
        let input = "```json\n{\"name\": \"test\"}\n```";
        assert_eq!(LlmExtractor::extract_json_block(input), r#"{"name": "test"}"#);
    }

    #[test]
    fn extract_json_block_with_plain_fence() {
        let input = "```\n{\"items\": [1, 2, 3]}\n```";
        assert_eq!(LlmExtractor::extract_json_block(input), r#"{"items": [1, 2, 3]}"#);
    }

    #[test]
    fn extract_json_block_with_surrounding_text() {
        let input = "Here is the data:\n```json\n{\"key\": \"value\"}\n```\nDone.";
        assert_eq!(LlmExtractor::extract_json_block(input), r#"{"key": "value"}"#);
    }

    #[test]
    fn extract_json_block_whitespace_trimming() {
        let input = "  \n  {\"a\": 1}  \n  ";
        assert_eq!(LlmExtractor::extract_json_block(input), r#"{"a": 1}"#);
    }

    #[test]
    fn build_system_prompt_with_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "title": {"type": "string"}
            }
        });
        let prompt = LlmExtractor::build_system_prompt(Some(&schema), None);
        assert!(prompt.contains("JSON schema"));
        assert!(prompt.contains("\"title\""));
    }

    #[test]
    fn build_system_prompt_with_instructions() {
        let prompt = LlmExtractor::build_system_prompt(None, Some("Extract prices only"));
        assert!(prompt.contains("Extract prices only"));
        assert!(prompt.contains("Additional instructions"));
    }

    #[test]
    fn build_system_prompt_with_both() {
        let schema = serde_json::json!({"type": "object"});
        let prompt = LlmExtractor::build_system_prompt(Some(&schema), Some("Be precise"));
        assert!(prompt.contains("JSON schema"));
        assert!(prompt.contains("Be precise"));
    }

    #[test]
    fn build_system_prompt_base_rules() {
        let prompt = LlmExtractor::build_system_prompt(None, None);
        assert!(prompt.contains("ONLY valid JSON"));
        assert!(prompt.contains("null"));
        assert!(prompt.contains("exact"));
    }

    #[test]
    fn new_extractor_without_key() {
        let config = AnthropicConfig {
            api_key: None,
            model: "test-model".into(),
        };
        let extractor = LlmExtractor::new(&config);
        assert!(extractor.api_key.is_none());
        assert_eq!(extractor.model, "test-model");
    }
}
