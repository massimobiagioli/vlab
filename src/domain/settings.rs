//! Provider, model, and credential settings (specification §5.1, ADR-0030).

use std::fmt;

/// Longest accepted model identifier, in bytes.
pub const MODEL_ID_MAX_LEN: usize = 64;

/// Supported AI providers. Only DeepSeek in the initial release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderId {
    DeepSeek,
}

impl ProviderId {
    pub fn parse(value: &str) -> Result<Self, SettingsError> {
        match value.trim() {
            "deepseek" => Ok(Self::DeepSeek),
            _ => Err(SettingsError::UnsupportedProvider),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::DeepSeek => "deepseek",
        }
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A model identifier: after trimming, 1–64 characters of
/// `[A-Za-z0-9._:-]`. Whether the provider accepts it is checked only when a
/// provider-backed command runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelId(String);

impl ModelId {
    pub fn parse(value: &str) -> Result<Self, SettingsError> {
        let value = value.trim();
        let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-');
        if value.is_empty() || value.len() > MODEL_ID_MAX_LEN || !value.chars().all(allowed) {
            return Err(SettingsError::InvalidModel);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Source-control providers. Only GitHub is supported for now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SourceControlProvider {
    #[default]
    Github,
}

impl SourceControlProvider {
    pub fn parse(value: &str) -> Result<Self, SettingsError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "github" => Ok(Self::Github),
            _ => Err(SettingsError::UnsupportedSourceControlProvider),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Github => "github",
        }
    }
}

impl std::fmt::Display for SourceControlProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Longest accepted credential, in bytes.
pub const API_KEY_MAX_BYTES: usize = 512;

/// Provider credentials loaded from the private settings file. Values are
/// never displayed: [`Credentials`] has a redacting `Debug`.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Credentials {
    pub deepseek_api_key: Option<String>,
    pub github_api_key: Option<String>,
}

impl Credentials {
    pub fn parse(
        deepseek_api_key: Option<String>,
        github_api_key: Option<String>,
    ) -> Result<Self, SettingsError> {
        Ok(Self {
            deepseek_api_key: valid_key(deepseek_api_key)?,
            github_api_key: valid_key(github_api_key)?,
        })
    }

    pub fn deepseek_present(&self) -> bool {
        self.deepseek_api_key.is_some()
    }

    pub fn github_present(&self) -> bool {
        self.github_api_key.is_some()
    }
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field(
                "deepseek_api_key",
                &self.deepseek_api_key.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "github_api_key",
                &self.github_api_key.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

fn valid_key(value: Option<String>) -> Result<Option<String>, SettingsError> {
    let Some(value) = value else { return Ok(None) };
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > API_KEY_MAX_BYTES
        || trimmed.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(SettingsError::InvalidCredential);
    }
    Ok(Some(trimmed.to_owned()))
}

/// Optional source-control configuration. GitHub is the default and the only
/// supported provider in the initial release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourceControl {
    pub provider: SourceControlProvider,
}

impl SourceControl {
    pub fn parse(provider: &str) -> Result<Self, SettingsError> {
        Ok(Self {
            provider: SourceControlProvider::parse(provider)?,
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Settings {
    pub provider: ProviderId,
    pub model: ModelId,
    pub source_control: SourceControl,
    pub credentials: Credentials,
}

impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("provider", &self.provider)
            .field("model", &self.model)
            .field("source_control", &self.source_control)
            .field("credentials", &self.credentials)
            .finish()
    }
}

impl Settings {
    /// Parses the provider/model pair and defaults source control to GitHub.
    pub fn parse(provider: &str, model: &str) -> Result<Self, SettingsError> {
        Ok(Self {
            provider: ProviderId::parse(provider)?,
            model: ModelId::parse(model)?,
            source_control: SourceControl::default(),
            credentials: Credentials::default(),
        })
    }

    pub fn with_source_control(
        provider: &str,
        model: &str,
        source_control: SourceControl,
    ) -> Result<Self, SettingsError> {
        Ok(Self {
            provider: ProviderId::parse(provider)?,
            model: ModelId::parse(model)?,
            source_control,
            credentials: Credentials::default(),
        })
    }

    pub fn with_credentials(
        provider: &str,
        model: &str,
        source_control: SourceControl,
        credentials: Credentials,
    ) -> Result<Self, SettingsError> {
        Ok(Self {
            provider: ProviderId::parse(provider)?,
            model: ModelId::parse(model)?,
            source_control,
            credentials,
        })
    }
}

/// Validation failures. Messages never repeat the rejected value, which may
/// be a misplaced credential.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SettingsError {
    #[error("`provider` must be `deepseek`")]
    UnsupportedProvider,
    #[error("`model` must be 1 to 64 letters, digits, `.`, `_`, `:`, or `-`")]
    InvalidModel,
    #[error("`source_control.provider` must be `github`")]
    UnsupportedSourceControlProvider,
    #[error("provider credentials must be non-empty tokens of at most 512 bytes")]
    InvalidCredential,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_settings_are_trimmed() {
        let settings = Settings::parse(" deepseek ", " deepseek-chat ").unwrap();
        assert_eq!(settings.provider, ProviderId::DeepSeek);
        assert_eq!(settings.model.as_str(), "deepseek-chat");
    }

    #[test]
    fn only_deepseek_is_supported() {
        for provider in ["", "openai", "DeepSeek", "deep seek"] {
            assert_eq!(
                ProviderId::parse(provider),
                Err(SettingsError::UnsupportedProvider)
            );
        }
    }

    #[test]
    fn model_identifiers_are_bounded_tokens() {
        assert!(ModelId::parse("vendor.model_v1:2-x").is_ok());
        assert!(ModelId::parse(&"m".repeat(64)).is_ok());
        let too_long = "m".repeat(65);
        for model in [
            "",
            "   ",
            "<supported-model-id>",
            "two words",
            "a/b",
            "é",
            too_long.as_str(),
        ] {
            assert_eq!(
                ModelId::parse(model),
                Err(SettingsError::InvalidModel),
                "{model:?}"
            );
        }
    }

    #[test]
    fn source_control_defaults_to_github_and_rejects_other_providers() {
        let settings = Settings::parse("deepseek", "deepseek-chat").unwrap();
        assert_eq!(
            settings.source_control.provider,
            SourceControlProvider::Github
        );
        assert_eq!(
            SourceControl::parse(" GitHub ").unwrap().provider.as_str(),
            "github"
        );
        assert_eq!(
            SourceControl::parse("gitlab").unwrap_err(),
            SettingsError::UnsupportedSourceControlProvider
        );
    }

    #[test]
    fn credentials_are_trimmed_validated_and_redacted() {
        let credentials =
            Credentials::parse(Some(" sk-test ".to_owned()), Some(" ghp-test ".to_owned()))
                .unwrap();
        assert_eq!(credentials.deepseek_api_key.as_deref(), Some("sk-test"));
        assert_eq!(credentials.github_api_key.as_deref(), Some("ghp-test"));
        assert!(credentials.deepseek_present());
        assert!(credentials.github_present());

        let debug = format!("{credentials:?}");
        assert!(!debug.contains("sk-test"), "{debug}");
        assert!(!debug.contains("ghp-test"), "{debug}");
        assert!(debug.contains("[REDACTED]"), "{debug}");

        let too_long = "k".repeat(API_KEY_MAX_BYTES + 1);
        for bad in ["", "   ", "two words", "bad\u{1b}", too_long.as_str()] {
            assert_eq!(
                Credentials::parse(Some(bad.to_owned()), None),
                Err(SettingsError::InvalidCredential),
                "{bad:?}"
            );
            if !bad.is_empty() {
                let error = Credentials::parse(Some(bad.to_owned()), None).unwrap_err();
                assert!(!error.to_string().contains(bad), "{bad:?}");
            }
        }
    }

    #[test]
    fn messages_do_not_repeat_values() {
        let secret = "sk-vlab-SECRET-MARKER";
        let error = Settings::parse(secret, "m").unwrap_err();
        assert!(!error.to_string().contains(secret));
    }
}
