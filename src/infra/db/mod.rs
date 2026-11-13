//! SQLite connection, pragmas, and private storage (ADR-0002, ADR-0010,
//! ADR-0016).
//!
//! One database file per user, opened with a single pooled connection. Every
//! connection has foreign keys enabled and a bounded busy timeout; WAL is
//! disabled. The file is created with mode `0600` and the parent directory is
//! verified private before anything is written.
//!
//! The API is synchronous and owns a current-thread Tokio runtime: the CLI is
//! a one-shot process, so blocking on each database operation keeps the
//! application layer free of async plumbing. [`Database::pool`] exposes the
//! underlying pool for tests and for the provider work in later packages.

pub mod agents;
pub mod app_state;
pub mod deletions;
pub mod directory;
pub mod events;
pub mod interactions;
pub mod kanban;
pub mod labs;
pub mod memories;
pub mod migrate;
pub mod planning;
pub mod policy;
pub mod project_agents;
pub mod projects;
pub mod workspace;

use std::fs::{self, OpenOptions};
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use sqlx::SqliteConnection;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};

use crate::app::storage::StorageError;
use crate::infra::paths::ensure_private_dir;

/// Bounded wait for another process to release the SQLite write lock.
pub const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Schema version this binary creates and supports (ADR-0016).
pub const SUPPORTED_SCHEMA_VERSION: i64 = 8;

struct Inner {
    runtime: tokio::runtime::Runtime,
    pool: SqlitePool,
}

/// A handle to the local database. Cloning shares one pool and runtime.
#[derive(Clone)]
pub struct Database {
    inner: Arc<Inner>,
    path: PathBuf,
}

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

impl Database {
    /// Opens the database with the default busy timeout, creating it if
    /// needed. Refuses a schema newer than [`SUPPORTED_SCHEMA_VERSION`].
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, StorageError> {
        Self::open_with(path, DEFAULT_BUSY_TIMEOUT)
    }

    /// Opens the database with an explicit busy timeout. Tests use short
    /// timeouts to exercise lock handling without slowing the suite.
    pub fn open_with(
        path: impl Into<PathBuf>,
        busy_timeout: Duration,
    ) -> Result<Self, StorageError> {
        let path = path.into();
        let parent = path
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| StorageError::Io {
                path: path.clone(),
                kind: io::ErrorKind::InvalidInput,
            })?;
        ensure_private_dir(&parent)
            .map_err(|_| StorageError::PermissionDenied { path: path.clone() })?;
        create_private_database_file(&path)?;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| StorageError::Io {
                path: path.clone(),
                kind: io::ErrorKind::Other,
            })?;

        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .foreign_keys(true)
            .busy_timeout(busy_timeout)
            .journal_mode(SqliteJournalMode::Delete)
            .synchronous(SqliteSynchronous::Full);

        let pool = runtime
            .block_on(
                SqlitePoolOptions::new()
                    .max_connections(1)
                    .connect_with(options),
            )
            .map_err(|error| map_sqlx_error(&path, &error))?;

        let database = Self {
            inner: Arc::new(Inner { runtime, pool }),
            path,
        };
        database.verify_connection(busy_timeout)?;
        database.verify_permissions()?;
        database.verify_schema_version()?;
        Ok(database)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The single-connection pool. Exposed for tests and later adapters; the
    /// application layer uses the typed repositories instead.
    pub fn pool(&self) -> &SqlitePool {
        &self.inner.pool
    }

    /// Runs a database future on the database's runtime.
    ///
    /// Must not be called from inside an async task on the same runtime; the
    /// synchronous repository methods are the supported entry points.
    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.inner.runtime.block_on(future)
    }

    /// Runs `body` inside one transaction. The transaction commits only if
    /// `body` succeeds; an error or a dropped future rolls it back.
    pub fn transaction<T>(
        &self,
        body: impl for<'c> FnOnce(&'c mut SqliteConnection) -> BoxFuture<'c, Result<T, StorageError>>,
    ) -> Result<T, StorageError> {
        self.block_on(async {
            let mut transaction = self
                .inner
                .pool
                .begin()
                .await
                .map_err(|error| map_sqlx_error(&self.path, &error))?;
            let value = body(&mut transaction).await?;
            // `Transaction` derefs to the connection; the borrow above ends
            // before commit, which consumes the transaction.
            transaction
                .commit()
                .await
                .map_err(|error| map_sqlx_error(&self.path, &error))?;
            Ok(value)
        })
    }

    fn verify_connection(&self, busy_timeout: Duration) -> Result<(), StorageError> {
        self.block_on(async {
            let mut connection = self
                .inner
                .pool
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(&self.path, &error))?;
            let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
                .fetch_one(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(&self.path, &error))?;
            let timeout: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
                .fetch_one(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(&self.path, &error))?;
            if foreign_keys != 1 {
                return Err(StorageError::Operation);
            }
            let expected = i64::try_from(busy_timeout.as_millis()).unwrap_or(i64::MAX);
            if timeout != expected {
                return Err(StorageError::Operation);
            }
            Ok(())
        })
    }

    #[cfg(unix)]
    fn verify_permissions(&self) -> Result<(), StorageError> {
        use std::os::unix::fs::PermissionsExt;

        // Tighten an existing file rather than refusing it: reducing access
        // is always safe, while the data may predate this build.
        fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600)).map_err(|error| {
            StorageError::Io {
                path: self.path.clone(),
                kind: error.kind(),
            }
        })?;
        let mode = fs::metadata(&self.path)
            .map_err(|error| StorageError::Io {
                path: self.path.clone(),
                kind: error.kind(),
            })?
            .permissions()
            .mode()
            & 0o777;
        if mode != 0o600 {
            return Err(StorageError::PermissionDenied {
                path: self.path.clone(),
            });
        }
        Ok(())
    }

    #[cfg(not(unix))]
    fn verify_permissions(&self) -> Result<(), StorageError> {
        Ok(())
    }

    fn verify_schema_version(&self) -> Result<(), StorageError> {
        let found = self.schema_version()?;
        if found > SUPPORTED_SCHEMA_VERSION {
            return Err(StorageError::NewerSchema {
                path: self.path.clone(),
                found,
                supported: SUPPORTED_SCHEMA_VERSION,
            });
        }
        Ok(())
    }

    /// `PRAGMA user_version`, maintained by the migration runner.
    pub fn schema_version(&self) -> Result<i64, StorageError> {
        self.block_on(async {
            let mut connection = self
                .inner
                .pool
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(&self.path, &error))?;
            sqlx::query_scalar("PRAGMA user_version")
                .fetch_one(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(&self.path, &error))
        })
    }

    /// Executes a batch of statements without parameters. Intended for tests
    /// and migrations; application queries use the typed repositories.
    pub fn execute(&self, sql: &'static str) -> Result<(), StorageError> {
        self.block_on(async {
            sqlx::raw_sql(sql)
                .execute(&self.inner.pool)
                .await
                .map(|_| ())
                .map_err(|error| map_sqlx_error(&self.path, &error))
        })
    }
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

/// Creates the database file with mode `0600`, refusing to follow a planted
/// link. An existing regular file is left in place.
fn create_private_database_file(path: &Path) -> Result<(), StorageError> {
    let io_error = |error: io::Error| StorageError::Io {
        path: path.to_path_buf(),
        kind: error.kind(),
    };
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(()),
        Ok(_) => Err(StorageError::PermissionDenied {
            path: path.to_path_buf(),
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(path) {
                Ok(_) => Ok(()),
                // Another process won the race; the file is still private.
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
                Err(error) => Err(io_error(error)),
            }
        }
        Err(error) => Err(io_error(error)),
    }
}

fn map_sqlx_error(path: &Path, error: &sqlx::Error) -> StorageError {
    let path = path.to_path_buf();
    match error {
        sqlx::Error::Database(database) => {
            let primary = database
                .code()
                .and_then(|code| code.parse::<i32>().ok())
                .map(|code| code & 0xff)
                .unwrap_or(0);
            match primary {
                // SQLITE_BUSY, SQLITE_LOCKED.
                5 | 6 => StorageError::Locked { path },
                // SQLITE_PERM, SQLITE_READONLY.
                3 | 8 => StorageError::PermissionDenied { path },
                // SQLITE_CORRUPT, SQLITE_NOTADB.
                11 | 26 => StorageError::Corrupt { path },
                // SQLITE_FULL.
                13 => StorageError::DiskFull { path },
                _ => StorageError::Operation,
            }
        }
        sqlx::Error::Io(error) => StorageError::Io {
            path,
            kind: error.kind(),
        },
        sqlx::Error::PoolTimedOut => StorageError::Locked { path },
        _ => StorageError::Operation,
    }
}
