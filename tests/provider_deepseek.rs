//! DeepSeek adapter contract and bounds (WP5-AC2).

use std::time::Duration;

use serde_json::json;
use vlab::domain::provider::{ModelProvider, ProviderError, ProviderRequest};
use vlab::infra::provider::deepseek::{AdapterLimits, DeepSeekProvider};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SECRET: &str = "sk-vlab-SECRET-MARKER";

fn request() -> ProviderRequest {
    ProviderRequest {
        system: "system".to_owned(),
        user: "user".to_owned(),
        model: "deepseek-chat".to_owned(),
        max_output_tokens: 100,
        json: true,
    }
}

fn provider(server: &MockServer, limits: AdapterLimits) -> DeepSeekProvider {
    DeepSeekProvider::new(format!("{}/chat/completions", server.uri()), SECRET, limits).unwrap()
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn success_body() -> serde_json::Value {
    json!({
        "id": "req-1",
        "model": "deepseek-chat",
        "choices": [{"message": {"content": "{\"ok\":true}"}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 3, "completion_tokens": 4}
    })
}

#[test]
fn a_successful_contract_response_is_parsed_and_the_key_stays_in_the_header() {
    let runtime = runtime();
    let server = runtime.block_on(MockServer::start());
    runtime.block_on(async {
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(success_body()))
            .mount(&server)
            .await;
    });
    let response = provider(&server, AdapterLimits::default())
        .complete(&request())
        .unwrap();
    assert_eq!(response.content, "{\"ok\":true}");
    assert_eq!(response.model, "deepseek-chat");
    assert_eq!(response.usage.unwrap().output_tokens, 4);

    let received = runtime.block_on(server.received_requests()).unwrap();
    let sent = &received[0];
    assert_eq!(
        sent.headers.get("authorization").unwrap().to_str().unwrap(),
        format!("Bearer {SECRET}")
    );
    let body = String::from_utf8(sent.body.clone()).unwrap();
    assert!(!body.contains(SECRET));
    assert!(body.contains("deepseek-chat"));
    assert!(!format!("{:?}", provider(&server, AdapterLimits::default())).contains(SECRET));
}

#[test]
fn status_codes_are_classified_without_body_leakage() {
    let runtime = runtime();
    for (status, expected) in [(401, "provider_authentication"), (500, "provider_server")] {
        let server = runtime.block_on(MockServer::start());
        runtime.block_on(async {
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(status).set_body_string(SECRET))
                .mount(&server)
                .await;
        });
        let error = provider(&server, AdapterLimits::default())
            .complete(&request())
            .unwrap_err();
        assert_eq!(error.code(), expected, "{status}");
        assert!(!format!("{error:?}").contains(SECRET));
    }
}

#[test]
fn rate_limit_retry_after_is_preserved() {
    let runtime = runtime();
    let server = runtime.block_on(MockServer::start());
    runtime.block_on(async {
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(429)
                    .insert_header("Retry-After", "7")
                    .set_body_string(SECRET),
            )
            .mount(&server)
            .await;
    });
    let error = provider(&server, AdapterLimits::default())
        .complete(&request())
        .unwrap_err();
    assert_eq!(
        error,
        ProviderError::RateLimited {
            retry_after: Some(Duration::from_secs(7))
        }
    );
}

#[test]
fn malformed_truncated_and_oversized_bodies_are_rejected() {
    let runtime = runtime();
    for (body, limits, expected) in [
        (
            "not json".to_owned(),
            AdapterLimits::default(),
            ProviderError::InvalidResponse,
        ),
        (
            "{\"choices\":[".to_owned(),
            AdapterLimits::default(),
            ProviderError::InvalidResponse,
        ),
        (
            "x".repeat(20),
            AdapterLimits {
                response_max_bytes: 10,
                ..AdapterLimits::default()
            },
            ProviderError::ResponseTooLarge,
        ),
    ] {
        let server = runtime.block_on(MockServer::start());
        runtime.block_on(async {
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_string(body))
                .mount(&server)
                .await;
        });
        let error = provider(&server, limits).complete(&request()).unwrap_err();
        assert_eq!(error, expected);
    }
}

#[test]
fn an_attempt_timeout_is_reported() {
    let runtime = runtime();
    let server = runtime.block_on(MockServer::start());
    runtime.block_on(async {
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(200))
                    .set_body_json(success_body()),
            )
            .mount(&server)
            .await;
    });
    let error = provider(
        &server,
        AdapterLimits {
            attempt_timeout: Duration::from_millis(30),
            ..AdapterLimits::default()
        },
    )
    .complete(&request())
    .unwrap_err();
    assert_eq!(error, ProviderError::Timeout);
}
