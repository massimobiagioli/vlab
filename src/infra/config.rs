//! The `settings.yaml` adapter (ADR-0015, ADR-0030).
//!
//! Parsing is strict and bounded, and error reasons are built from typed
//! parser information only, so no text from the file can reach the user.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_saphyr::Error as YamlError;

use crate::app::AppError;
use crate::app::settings::{LoadError, SettingsDraft, SettingsStore};
use crate::domain::settings::Settings;
use crate::infra::paths::{create_private_file, ensure_private_dir};

/// Largest accepted settings file.
pub const MAX_SETTINGS_BYTES: usize = 64 * 1024;

const DRAFT_PREFIX: &str = ".settings.yaml.";
const DRAFT_SUFFIX: &str = ".tmp";

/// Written to the draft when no settings file exists yet. The placeholder
/// model fails validation, so the template cannot be saved unchanged.
pub const TEMPLATE: &str = "\
# VLab settings. They are saved only if they are valid.
#
# provider: the AI provider. Only \"deepseek\" is supported.
# model:    a model identifier accepted by the provider, taken from the
#           DeepSeek API documentation. Replace the placeholder below.
#
# This file contains credentials. Keep it private and outside version
# control. VLab writes it with mode 0600.
provider: deepseek
model: \"<supported-model-id>\"
credentials:
  deepseek_api_key: \"sk-your-deepseek-key\"
  github_api_key: \"ghp-your-github-token\"
";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSettings {
    provider: String,
    model: String,
    source_control: Option<RawSourceControl>,
    credentials: Option<RawCredentials>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCredentials {
    deepseek_api_key: Option<String>,
    github_api_key: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSourceControl {
    provider: String,
}

/// Parses and validates a settings document. Pure apart from its input.
pub fn parse_settings(bytes: &[u8]) -> Result<Settings, String> {
    if bytes.len() > MAX_SETTINGS_BYTES {
        return Err(format!(
            "it is larger than {} KiB",
            MAX_SETTINGS_BYTES / 1024
        ));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "it is not valid UTF-8".to_owned())?;
    let raw: RawSettings = serde_saphyr::from_str(text).map_err(|error| {
        // A syntactically valid document whose values have the wrong type
        // deserves a different message than broken YAML. Parsing the same
        // text without the schema tells the two apart.
        let syntactically_valid = serde_saphyr::from_str::<serde::de::IgnoredAny>(text).is_ok();
        let inner = error.without_snippet();
        if syntactically_valid {
            type_reason(inner)
        } else {
            yaml_reason(inner)
        }
    })?;
    let source_control = match raw.source_control {
        Some(raw_source) => crate::domain::settings::SourceControl::parse(&raw_source.provider)
            .map_err(|error| error.to_string())?,
        None => crate::domain::settings::SourceControl::default(),
    };
    let credentials = match raw.credentials {
        Some(raw_credentials) => crate::domain::settings::Credentials::parse(
            raw_credentials.deepseek_api_key,
            raw_credentials.github_api_key,
        )
        .map_err(|error| error.to_string())?,
        None => crate::domain::settings::Credentials::default(),
    };
    Settings::with_credentials(&raw.provider, &raw.model, source_control, credentials)
        .map_err(|error| error.to_string())
}

/// Explains a schema/type mismatch in a syntactically valid document.
fn type_reason(error: &YamlError) -> String {
    match error {
        YamlError::SerdeUnknownField { .. }
        | YamlError::SerdeMissingField { .. }
        | YamlError::SerdeInvalidValue { .. }
        | YamlError::Message { .. }
        | YamlError::Eof { .. }
        | YamlError::Budget { .. }
        | YamlError::MultipleDocuments { .. } => yaml_reason(error),
        _ => "`provider` and `model` must be text values".to_owned(),
    }
}

/// A content-free description of a parse failure.
///
/// Errors may be wrapped to carry a source snippet; the snippet contains the
/// edited YAML, including any misplaced secret, so only the inner error is
/// inspected and only typed field names are repeated.
fn yaml_reason(error: &YamlError) -> String {
    let error = error.without_snippet();
    let line = error
        .location()
        .map(|location| location.line())
        .filter(|line| *line > 0);
    let at = |line: Option<u64>| {
        line.map(|line| format!(" (line {line})"))
            .unwrap_or_default()
    };
    match error {
        YamlError::SerdeUnknownField { .. } => format!(
            "it contains an unknown field{}; only `provider`, `model`, `source_control`, and `credentials` are allowed",
            at(line)
        ),
        YamlError::SerdeMissingField { field, .. } => {
            format!("the required field `{field}` is missing")
        }
        YamlError::SerdeInvalidType { .. } | YamlError::SerdeInvalidValue { .. } => {
            format!("`provider` and `model` must be text values{}", at(line))
        }
        YamlError::MultipleDocuments { .. } => "it contains more than one YAML document".to_owned(),
        YamlError::Eof { .. } => "it is empty or incomplete".to_owned(),
        YamlError::Budget { .. } => "it exceeds the YAML complexity limits".to_owned(),
        _ => format!("it is not valid YAML{}", at(line)),
    }
}

/// `settings.yaml` in the configuration directory.
#[derive(Debug, Clone)]
pub struct SettingsFile {
    dir: PathBuf,
    path: PathBuf,
}

impl SettingsFile {
    pub fn new(dir: &Path, path: PathBuf) -> Self {
        Self {
            dir: dir.to_path_buf(),
            path,
        }
    }

    fn read_bounded(path: &Path) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        // One byte over the limit is enough to detect an oversized file.
        File::open(path)?
            .take(MAX_SETTINGS_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    /// Removes drafts left behind by an interrupted edit.
    fn remove_stale_drafts(&self) {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let is_draft = name.starts_with(DRAFT_PREFIX) && name.ends_with(DRAFT_SUFFIX);
            if is_draft && entry.file_type().is_ok_and(|kind| kind.is_file()) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

fn load_from(path: &Path) -> Result<Settings, LoadError> {
    // The file may hold credentials: fail closed when group or other users can
    // read it. A missing file is reported separately so the user gets the
    // create-it hint.
    match fs::metadata(path) {
        Ok(metadata) => verify_private_file(path, &metadata)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(LoadError::Missing),
        Err(error) => return Err(LoadError::Unreadable(error.kind())),
    }
    let bytes =
        SettingsFile::read_bounded(path).map_err(|error| LoadError::Unreadable(error.kind()))?;
    parse_settings(&bytes).map_err(LoadError::Invalid)
}

#[cfg(unix)]
fn verify_private_file(_path: &Path, metadata: &fs::Metadata) -> Result<(), LoadError> {
    use std::os::unix::fs::PermissionsExt;
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(LoadError::Exposed);
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_private_file(_path: &Path, _metadata: &fs::Metadata) -> Result<(), LoadError> {
    Ok(())
}

impl SettingsStore for SettingsFile {
    type Draft = Draft;

    fn path(&self) -> &Path {
        &self.path
    }

    fn load(&self) -> Result<Settings, LoadError> {
        load_from(&self.path)
    }

    fn create_draft(&self) -> Result<Draft, AppError> {
        ensure_private_dir(&self.dir)?;
        self.remove_stale_drafts();

        let initial = match Self::read_bounded(&self.path) {
            Ok(bytes) if bytes.len() > MAX_SETTINGS_BYTES => {
                return Err(AppError::Configuration {
                    problem: format!(
                        "the settings file `{}` is larger than {} KiB.",
                        self.path.display(),
                        MAX_SETTINGS_BYTES / 1024
                    ),
                    hint: "Move it away; `vlab --edit-settings` will then start from a template."
                        .to_owned(),
                });
            }
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => TEMPLATE.as_bytes().to_vec(),
            Err(error) => {
                return Err(AppError::Io {
                    context: "reading the settings file",
                    kind: error.kind(),
                });
            }
        };

        let name = format!(
            "{DRAFT_PREFIX}{}{DRAFT_SUFFIX}",
            uuid::Uuid::new_v4().simple()
        );
        let draft = Draft {
            path: self.dir.join(name),
            dir: self.dir.clone(),
            target: self.path.clone(),
            committed: false,
        };
        let mut file = create_private_file(&draft.path)?;
        file.write_all(&initial)
            .and_then(|()| file.sync_all())
            .map_err(|error| AppError::Io {
                context: "writing the settings draft",
                kind: error.kind(),
            })?;
        Ok(draft)
    }
}

/// A private `0600` copy next to the settings file, deleted on drop unless
/// committed.
#[derive(Debug)]
pub struct Draft {
    path: PathBuf,
    dir: PathBuf,
    target: PathBuf,
    committed: bool,
}

impl SettingsDraft for Draft {
    fn path(&self) -> &Path {
        &self.path
    }

    fn load(&self) -> Result<Settings, LoadError> {
        load_from(&self.path)
    }

    fn commit(mut self) -> Result<(), AppError> {
        let io_error = |context| {
            move |error: io::Error| AppError::Io {
                context,
                kind: error.kind(),
            }
        };
        // The editor may have replaced the file: flush whatever is there now.
        File::open(&self.path)
            .and_then(|file| file.sync_all())
            .map_err(io_error("flushing the settings draft"))?;
        fs::rename(&self.path, &self.target).map_err(io_error("replacing the settings file"))?;
        self.committed = true;
        sync_dir(&self.dir).map_err(io_error("flushing the settings directory"))
    }
}

impl Drop for Draft {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(unix)]
fn sync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

#[cfg(not(unix))]
fn sync_dir(_dir: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "sk-vlab-SECRET-MARKER";

    fn fixture(name: &str) -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name);
        fs::read(path).unwrap()
    }

    #[test]
    fn valid_fixtures_load() {
        for name in [
            "settings-valid.yaml",
            "settings-valid-quoted.yaml",
            "settings-credentials.yaml",
        ] {
            let settings = parse_settings(&fixture(name)).unwrap();
            assert_eq!(settings.provider.as_str(), "deepseek");
            assert_eq!(settings.model.as_str(), "deepseek-chat");
        }
        let with_keys = parse_settings(&fixture("settings-credentials.yaml")).unwrap();
        assert_eq!(
            with_keys.credentials.deepseek_api_key.as_deref(),
            Some("sk-test-deepseek")
        );
        assert_eq!(
            with_keys.credentials.github_api_key.as_deref(),
            Some("ghp-test-github")
        );
    }

    #[test]
    fn every_invalid_fixture_has_a_specific_content_free_reason() {
        let cases = [
            (
                "settings-unknown-field.yaml",
                "it contains an unknown field (line 3); only `provider`, `model`, `source_control`, and `credentials` are allowed",
            ),
            ("settings-malformed.yaml", "it is not valid YAML"),
            (
                "settings-wrong-type.yaml",
                "`provider` and `model` must be text values",
            ),
            (
                "settings-unknown-provider.yaml",
                "`provider` must be `deepseek`",
            ),
            (
                "settings-empty-model.yaml",
                "`model` must be 1 to 64 letters, digits, `.`, `_`, `:`, or `-`",
            ),
            (
                "settings-missing-model.yaml",
                "the required field `model` is missing",
            ),
            ("settings-duplicate-key.yaml", "it is not valid YAML"),
            (
                "settings-two-documents.yaml",
                "it contains more than one YAML document",
            ),
        ];
        for (name, expected) in cases {
            let reason = parse_settings(&fixture(name)).unwrap_err();
            assert!(reason.starts_with(expected), "{name}: {reason}");
            assert!(!reason.contains(SECRET), "{name}: {reason}");
        }
    }

    #[test]
    fn source_control_is_optional_and_accepts_only_github() {
        let default = parse_settings(fixture("settings-valid.yaml").as_slice()).unwrap();
        assert_eq!(
            default.source_control.provider,
            crate::domain::settings::SourceControlProvider::Github
        );
        let explicit = parse_settings(fixture("settings-github.yaml").as_slice()).unwrap();
        assert_eq!(
            explicit.source_control.provider,
            crate::domain::settings::SourceControlProvider::Github
        );
    }

    #[test]
    fn size_encoding_and_emptiness_are_checked() {
        let big = format!(
            "provider: deepseek\nmodel: m\n#{}\n",
            "x".repeat(MAX_SETTINGS_BYTES)
        );
        assert_eq!(
            parse_settings(big.as_bytes()).unwrap_err(),
            "it is larger than 64 KiB"
        );
        assert_eq!(
            parse_settings(b"provider: \xff\n").unwrap_err(),
            "it is not valid UTF-8"
        );
        assert!(parse_settings(b"").is_err());
    }

    #[test]
    fn the_template_is_valid_yaml_but_not_valid_settings() {
        assert!(TEMPLATE.contains("provider: deepseek"));
        assert_eq!(
            parse_settings(TEMPLATE.as_bytes()).unwrap_err(),
            "`model` must be 1 to 64 letters, digits, `.`, `_`, `:`, or `-`"
        );
    }
}
