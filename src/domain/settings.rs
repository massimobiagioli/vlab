//! Provider and model settings (specification §5.1, ADR-0015).

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub provider: ProviderId,
    pub model: ModelId,
    pub source_control: SourceControl,
}

impl Settings {
    /// Parses the provider/model pair and defaults source control to GitHub.
    pub fn parse(provider: &str, model: &str) -> Result<Self, SettingsError> {
        Ok(Self {
            provider: ProviderId::parse(provider)?,
            model: ModelId::parse(model)?,
            source_control: SourceControl::default(),
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
    fn messages_do_not_repeat_values() {
        let secret = "sk-vlab-SECRET-MARKER";
        let error = Settings::parse(secret, "m").unwrap_err();
        assert!(!error.to_string().contains(secret));
    }
}
