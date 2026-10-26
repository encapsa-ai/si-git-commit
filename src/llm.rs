use crate::config::{Config, Provider};
use crate::error::{Result, SiError};
use crate::prompt::{build_user_prompt, parse_commit_message, SYSTEM_PROMPT};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
struct ChatMessage {
    role: String,
    content: String,
}

/// OpenAI Chat Completions request using only current parameters.
///
/// Modern models (GPT-5 family, o-series, and late GPT-4.1) reject the
/// legacy `max_tokens` field and custom `temperature` values. This struct
/// therefore sends `max_completion_tokens` and lets each model use its
/// default temperature — the exact fix for:
/// `Unsupported parameter: 'max_tokens' is not supported with this model.
///  Use 'max_completion_tokens' instead.`
#[derive(Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_completion_tokens: u32,
}

#[derive(Deserialize)]
struct OpenAiChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChatChoice>,
}

/// Anthropic Messages request. Anthropic's current parameter is
/// `max_tokens`, so it is kept here — per-provider parameter names.
#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ChatMessage>,
}

#[derive(Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
}

pub struct Llm {
    config: Config,
}

impl Llm {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn model(&self) -> &str {
        &self.config.model
    }

    pub async fn gen_commit_message(
        &self,
        diff: &str,
        stat: &str,
    ) -> Result<(String, Option<String>)> {
        let prompt = build_user_prompt(diff, stat, self.config.message_level);
        let response = self.call_provider(&prompt).await?;
        Ok(parse_commit_message(&response))
    }

    async fn call_provider(&self, prompt: &str) -> Result<String> {
        match self.config.provider {
            Provider::Openai | Provider::OpenAiCompatible => {
                self.call_openai_compatible(prompt).await
            }
            Provider::Anthropic => self.call_anthropic(prompt).await,
        }
    }

    fn http_client(&self) -> Result<reqwest::Client> {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(SiError::from)
    }

    async fn call_openai_compatible(&self, prompt: &str) -> Result<String> {
        let api_key = self.config.require_api_key()?;
        let base = self.config.effective_base_url();
        let url = format!("{base}/chat/completions");

        let request = OpenAiChatRequest {
            model: self.config.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: SYSTEM_PROMPT.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: prompt.to_string(),
                },
            ],
            max_completion_tokens: self.config.max_completion_tokens,
        };

        let client = self.http_client()?;
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SiError::Api {
                provider: self.config.provider.display_name().to_string(),
                message: format!("{status}: {error_text}"),
            });
        }

        let chat_response: OpenAiChatResponse = response.json().await?;
        chat_response
            .choices
            .first()
            .map(|choice| choice.message.content.clone())
            .filter(|c| !c.trim().is_empty())
            .ok_or_else(|| SiError::Api {
                provider: self.config.provider.display_name().to_string(),
                message: "empty response (no choices returned)".to_string(),
            })
    }

    async fn call_anthropic(&self, prompt: &str) -> Result<String> {
        let api_key = self.config.require_api_key()?;
        let base = self.config.effective_base_url();
        let url = format!("{base}/messages");

        let request = AnthropicRequest {
            model: self.config.model.clone(),
            max_tokens: self.config.max_completion_tokens,
            system: SYSTEM_PROMPT.to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let client = self.http_client()?;
        let response = client
            .post(&url)
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SiError::Api {
                provider: "Anthropic".to_string(),
                message: format!("{status}: {error_text}"),
            });
        }

        let parsed: AnthropicResponse = response.json().await?;
        let text = parsed
            .content
            .into_iter()
            .filter(|b| b.kind == "text")
            .map(|b| b.text)
            .collect::<Vec<_>>()
            .join("");
        if text.trim().is_empty() {
            return Err(SiError::Api {
                provider: "Anthropic".to_string(),
                message: "empty response (no text blocks returned)".to_string(),
            });
        }
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_request_uses_modern_parameters_only() {
        // Covers both a still-supported 4.1-class model and a current
        // GPT-5-class model: the payload must be identical in shape.
        for model in ["gpt-4.1-mini", "gpt-5-mini", "gpt-5.6-luna"] {
            let request = OpenAiChatRequest {
                model: model.to_string(),
                messages: vec![ChatMessage {
                    role: "user".to_string(),
                    content: "Write a commit message.".to_string(),
                }],
                max_completion_tokens: 800,
            };

            let body = serde_json::to_value(&request).expect("request should serialize");

            assert_eq!(body["model"].as_str(), Some(model));
            assert_eq!(body["max_completion_tokens"].as_u64(), Some(800));
            assert!(
                body.get("max_tokens").is_none(),
                "legacy max_tokens must never be sent to OpenAI for {model}"
            );
            assert!(
                body.get("temperature").is_none(),
                "temperature must use the model default for {model}"
            );
        }
    }

    #[test]
    fn anthropic_request_uses_max_tokens() {
        let request = AnthropicRequest {
            model: "claude-sonnet-4-6".to_string(),
            max_tokens: 800,
            system: SYSTEM_PROMPT.to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "hi".to_string(),
            }],
        };
        let body = serde_json::to_value(&request).expect("request should serialize");
        assert_eq!(body["max_tokens"].as_u64(), Some(800));
        assert!(body.get("max_completion_tokens").is_none());
    }
}
