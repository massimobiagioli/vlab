//! DeepSeek chat-completions adapter (ADR-0026).
//!
//! The adapter is the only place that knows the HTTP envelope. It returns no
//! response body in errors and never exposes the credential in Debug.

use std::io::Read;
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, RETRY_AFTER};
use serde::Deserialize;
use serde_json::json;

use crate::domain::provider::{
    ModelProvider, ProviderError, ProviderRequest, ProviderResponse, Usage,
};
use crate::obs::Redacted;

pub const DEFAULT_ENDPOINT: &str = "https://api.deepseek.com/chat/completions";

#[derive(Debug, Clone, Copy)]
pub struct AdapterLimits {
    pub attempt_timeout: Duration,
    pub connect_timeout: Duration,
    pub response_max_bytes: usize,
}

impl Default for AdapterLimits {
    fn default() -> Self {
        Self {
            attempt_timeout: Duration::from_secs(60),
            connect_timeout: Duration::from_secs(10),
            response_max_bytes: 256 * 1024,
        }
    }
}

pub struct DeepSeekProvider {
    client: Client,
    endpoint: String,
    api_key: Redacted<String>,
    limits: AdapterLimits,
}

impl DeepSeekProvider {
    pub fn from_env(
        endpoint: impl Into<String>,
        limits: AdapterLimits,
    ) -> Result<Self, ProviderError> {
        let api_key = std::env::var("DEEPSEEK_API_KEY")
            .ok()
            .filter(|value| !value.is_empty())
            .ok_or(ProviderError::Configuration)?;
        Self::new(endpoint, api_key, limits)
    }

    pub fn new(
        endpoint: impl Into<String>,
        api_key: impl Into<String>,
        limits: AdapterLimits,
    ) -> Result<Self, ProviderError> {
        let client = Client::builder()
            .timeout(limits.attempt_timeout)
            .connect_timeout(limits.connect_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| ProviderError::Configuration)?;
        Ok(Self {
            client,
            endpoint: endpoint.into(),
            api_key: Redacted::new(api_key.into()),
            limits,
        })
    }
}

impl std::fmt::Debug for DeepSeekProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeepSeekProvider")
            .field("endpoint", &self.endpoint)
            .field("api_key", &self.api_key)
            .field("limits", &self.limits)
            .finish()
    }
}

#[derive(Deserialize)]
struct ChatResponse {
    id: Option<String>,
    model: Option<String>,
    choices: Vec<Choice>,
    usage: Option<RawUsage>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct Message {
    content: Option<String>,
}

#[derive(Deserialize)]
struct RawUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
}

impl ModelProvider for DeepSeekProvider {
    fn complete(&self, request: &ProviderRequest) -> Result<ProviderResponse, ProviderError> {
        let mut body = json!({
            "model": request.model,
            "messages": [
                {"role": "system", "content": request.system},
                {"role": "user", "content": request.user},
            ],
            "max_tokens": request.max_output_tokens,
        });
        if request.json {
            body["response_format"] = json!({"type": "json_object"});
        }

        let response = self
            .client
            .post(&self.endpoint)
            .header(AUTHORIZATION, format!("Bearer {}", self.api_key.expose()))
            .json(&body)
            .send()
            .map_err(map_transport)?;
        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::Authentication);
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .map(Duration::from_secs);
            return Err(ProviderError::RateLimited { retry_after });
        }
        if status.is_server_error() {
            return Err(ProviderError::Server {
                status: status.as_u16(),
                request_id,
            });
        }
        if !status.is_success() {
            return Err(ProviderError::InvalidResponse);
        }

        let mut bytes = Vec::new();
        response
            .take(self.limits.response_max_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ProviderError::Transport)?;
        if bytes.len() > self.limits.response_max_bytes {
            return Err(ProviderError::ResponseTooLarge);
        }
        let parsed: ChatResponse =
            serde_json::from_slice(&bytes).map_err(|_| ProviderError::InvalidResponse)?;
        let ChatResponse {
            id,
            model,
            choices,
            usage,
        } = parsed;
        let choice = choices
            .into_iter()
            .next()
            .ok_or(ProviderError::InvalidResponse)?;
        let content = choice
            .message
            .content
            .ok_or(ProviderError::InvalidResponse)?;
        Ok(ProviderResponse {
            content,
            model: model.unwrap_or_else(|| request.model.clone()),
            usage: usage.map(|usage| Usage {
                input_tokens: usage.prompt_tokens.unwrap_or(0),
                output_tokens: usage.completion_tokens.unwrap_or(0),
            }),
            finish_reason: choice.finish_reason,
            request_id: id.or(request_id),
        })
    }
}

fn map_transport(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::Timeout
    } else {
        ProviderError::Transport
    }
}
