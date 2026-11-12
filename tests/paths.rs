//! Path resolution and private storage (WP1-AC5).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use vlab::infra::paths::{PathError, Paths, PlatformDirs};

fn platform() -> Option<PlatformDirs> {
    Some(PlatformDirs {
        config: PathBuf::from("/base/config"),
        data: PathBuf::from("/base/data"),
    })
}

fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
    let pairs: Vec<(String, OsString)> = pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), OsString::from(value)))
        .collect();
    move |key| {
        pairs
            .iter()
            .find(|(candidate, _)| candidate == key)
            .map(|(_, value)| value.clone())
    }
}

#[test]
fn defaults_are_per_user_platform_directories() {
    let paths = Paths::resolve(env(&[]), platform()).unwrap();
    assert_eq!(paths.config_dir(), Path::new("/base/config/vlab"));
    assert_eq!(
        paths.settings_file(),
        Path::new("/base/config/vlab/settings.yaml")
    );
    assert_eq!(paths.data_dir(), Path::new("/base/data/vlab"));
    assert_eq!(paths.database_file(), Path::new("/base/data/vlab/vlab.db"));
}

#[test]
fn workspace_is_next_to_dot_data_or_under_the_data_directory() {
    let paths = Paths::resolve(env(&[]), platform()).unwrap();
    assert_eq!(
        paths.workspace_dir(),
        Path::new("/base/data/vlab/workspace")
    );

    let lookup = env(&[("VLAB_DATA_DIR", "/repo/.data")]);
    let paths = Paths::resolve(lookup, platform()).unwrap();
    assert_eq!(paths.workspace_dir(), Path::new("/repo/.workspace"));
}

#[test]
fn overrides_are_honored() {
    let lookup = env(&[
        ("VLAB_CONFIG_DIR", "/o/config"),
        ("VLAB_DATA_DIR", "/o/data"),
    ]);
    let paths = Paths::resolve(lookup, platform()).unwrap();
    assert_eq!(paths.settings_file(), Path::new("/o/config/settings.yaml"));
    assert_eq!(paths.database_file(), Path::new("/o/data/vlab.db"));

    let lookup = env(&[("VLAB_DATA_DIR", "/o/data"), ("VLAB_DB_PATH", "/db/x.db")]);
    let paths = Paths::resolve(lookup, platform()).unwrap();
    assert_eq!(paths.data_dir(), Path::new("/o/data"));
    assert_eq!(paths.database_file(), Path::new("/db/x.db"));
}

#[test]
fn overrides_do_not_need_platform_directories() {
    let lookup = env(&[
        ("VLAB_CONFIG_DIR", "/o/config"),
        ("VLAB_DATA_DIR", "/o/data"),
    ]);
    assert!(Paths::resolve(lookup, None).is_ok());
}

#[test]
fn relative_or_empty_overrides_are_rejected() {
    for (variable, value) in [
        ("VLAB_CONFIG_DIR", "config"),
        ("VLAB_DATA_DIR", "./data"),
        ("VLAB_DB_PATH", "vlab.db"),
        ("VLAB_CONFIG_DIR", ""),
    ] {
        let error = Paths::resolve(env(&[(variable, value)]), platform()).unwrap_err();
        assert!(
            matches!(error, PathError::InvalidOverride { variable: v } if v == variable),
            "{variable}={value:?}: {error:?}"
        );
    }
}

#[test]
fn missing_platform_directories_without_overrides_fail() {
    let error = Paths::resolve(env(&[]), None).unwrap_err();
    assert!(matches!(error, PathError::NoUserDirectory), "{error:?}");
}

#[cfg(unix)]
mod unix {
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    use vlab::infra::paths::{PathError, create_private_file, ensure_private_dir};

    fn mode(path: &std::path::Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn private_directories_are_created_with_mode_0700() {
        let root = tempfile::tempdir().unwrap();
        let leaf = root.path().join("a").join("b");
        ensure_private_dir(&leaf).unwrap();
        assert_eq!(mode(&leaf), 0o700);
        assert_eq!(mode(&root.path().join("a")), 0o700);
        // Idempotent on an existing private directory.
        ensure_private_dir(&leaf).unwrap();
    }

    #[test]
    fn existing_directories_open_to_others_are_refused() {
        let root = tempfile::tempdir().unwrap();
        let open = root.path().join("open");
        fs::create_dir(&open).unwrap();
        fs::set_permissions(&open, fs::Permissions::from_mode(0o755)).unwrap();
        let error = ensure_private_dir(&open).unwrap_err();
        assert!(matches!(error, PathError::NotPrivate { .. }), "{error:?}");
        assert_eq!(mode(&open), 0o755, "permissions must not be changed");
    }

    #[test]
    fn files_and_symlinks_in_place_of_a_directory_are_refused() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("file");
        fs::write(&file, b"").unwrap();
        let error = ensure_private_dir(&file).unwrap_err();
        assert!(
            matches!(error, PathError::NotADirectory { .. }),
            "{error:?}"
        );

        let target = root.path().join("target");
        ensure_private_dir(&target).unwrap();
        let link = root.path().join("link");
        symlink(&target, &link).unwrap();
        let error = ensure_private_dir(&link).unwrap_err();
        assert!(
            matches!(error, PathError::NotADirectory { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn private_files_are_created_with_mode_0600_and_never_reused() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("settings.yaml");
        drop(create_private_file(&path).unwrap());
        assert_eq!(mode(&path), 0o600);
        let error = create_private_file(&path).unwrap_err();
        assert!(matches!(error, PathError::Io { .. }), "{error:?}");
    }
}
