//! The provider client is confined to the adapter.
//!
//! WP-5 adds reqwest for DeepSeek; the domain port and the rest of the
//! application never see an HTTP type.

#[test]
fn the_http_client_is_confined_to_the_provider_adapter() {
    let manifest = include_str!("../Cargo.toml");
    assert!(manifest.contains("reqwest"));
    let domain = include_str!("../src/domain/provider.rs");
    assert!(!domain.contains("reqwest"));
    let prompt = include_str!("../src/app/planning/prompt.rs");
    assert!(!prompt.contains("reqwest"));
}
