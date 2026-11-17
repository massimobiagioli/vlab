//! Markdown document import and references (ADR-0059).
//!
//! Documents are copied into the data file store; the database keeps only the
//! scope, title, and relative path.

use std::path::Path;

use crate::app::AppError;
use crate::app::directory::{AgentRecord, Directory};
use crate::app::projects::ProjectStore;
use crate::app::storage::StorageError;
use crate::domain::ids::{DocumentId, UniqueIdError, unique_short_id};
use crate::domain::memory::MemoryScope;
use crate::infra::files::{FileStore, MAX_DOCUMENT_BYTES};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub id: DocumentId,
    pub scope: MemoryScope,
    pub path: String,
    pub title: String,
}

pub trait DocumentStore {
    fn insert_document(&self, document: &Document) -> Result<(), StorageError>;

    fn document_exists(&self, id: &DocumentId) -> Result<bool, StorageError>;

    /// Allocates the stable entity folder and returns the relative file path.
    fn document_path(&self, id: &DocumentId, scope: &MemoryScope) -> Result<String, StorageError>;
}

/// Imports a markdown file into a lab, project, or agent document folder.
///
/// `into` is `lab/<name>`, `project/<name>`, or `agent`. The agent form needs
/// `agent` to carry an agent name or identifier.
pub fn import(
    store: &(impl DocumentStore + Directory + ProjectStore),
    files: &FileStore,
    source: &Path,
    into: &str,
    agent: Option<&str>,
) -> Result<Document, AppError> {
    let (scope, _label) = resolve_scope(store, into, agent)?;
    let title = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.chars().take(100).collect::<String>())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or_else(|| "document".to_owned());
    let generated = unique_short_id(|candidate| match DocumentId::parse(candidate) {
        Some(id) => store.document_exists(&id),
        None => Ok(true),
    })
    .map_err(map_unique)?;
    let id = DocumentId::parse(&generated).ok_or(AppError::Internal)?;
    let path = store.document_path(&id, &scope)?;
    files
        .import(source, &path, MAX_DOCUMENT_BYTES)
        .map_err(AppError::from)?;
    let document = Document {
        id: id.clone(),
        scope,
        path: path.clone(),
        title,
    };
    if let Err(error) = store.insert_document(&document) {
        let _ = files.delete(&path);
        return Err(error.into());
    }
    Ok(document)
}

fn resolve_scope(
    store: &(impl Directory + ProjectStore),
    into: &str,
    agent: Option<&str>,
) -> Result<(MemoryScope, String), AppError> {
    if into == "agent" {
        let raw = agent.ok_or_else(|| AppError::Usage {
            message: "importing into an agent needs `AGENT=<name-or-id>`.".to_owned(),
        })?;
        let record = resolve_agent(store, raw)?;
        return Ok((MemoryScope::Agent(record.id.clone()), record.name.clone()));
    }
    if let Some(name) = into.strip_prefix("lab/") {
        let lab = resolve_lab(store, name)?;
        return Ok((MemoryScope::Lab(lab.0), lab.1));
    }
    if let Some(name) = into.strip_prefix("project/") {
        let project = resolve_project(store, name)?;
        return Ok((MemoryScope::Project(project.0), project.1));
    }
    Err(AppError::Usage {
        message: "`--into` must be `agent`, `lab/<name>`, or `project/<name>`.".to_owned(),
    })
}

fn resolve_lab(
    store: &impl Directory,
    name: &str,
) -> Result<(crate::domain::ids::LabId, String), AppError> {
    let matches: Vec<_> = store
        .labs()?
        .into_iter()
        .filter(|lab| lab.name.eq_ignore_ascii_case(name.trim()))
        .collect();
    match matches.as_slice() {
        [] => Err(AppError::Precondition {
            problem: format!("no laboratory named `{name}` exists."),
            hint: "Check the laboratory name or create it first.".to_owned(),
        }),
        [lab] => Ok((lab.id.clone(), lab.name.clone())),
        _ => Err(AppError::Precondition {
            problem: format!("more than one laboratory is named `{name}`."),
            hint: "Rename one laboratory or use a unique name.".to_owned(),
        }),
    }
}

fn resolve_project(
    store: &(impl Directory + ProjectStore),
    name: &str,
) -> Result<(crate::domain::ids::ProjectId, String), AppError> {
    let mut matches = Vec::new();
    for lab in store.labs()? {
        for project in store.projects(&lab.id)? {
            if project.name.eq_ignore_ascii_case(name.trim()) {
                matches.push((project.id, project.name));
            }
        }
    }
    match matches.as_slice() {
        [] => Err(AppError::Precondition {
            problem: format!("no project named `{name}` exists."),
            hint: "Check the project name or create it first.".to_owned(),
        }),
        [project] => Ok(project.clone()),
        _ => Err(AppError::Precondition {
            problem: format!("more than one project is named `{name}`."),
            hint: "Rename one project or use a unique name.".to_owned(),
        }),
    }
}

fn resolve_agent(store: &impl Directory, raw: &str) -> Result<AgentRecord, AppError> {
    let agents = store.all_agents()?;
    if let Some(id) = crate::domain::ids::AgentId::parse(raw.trim()) {
        return agents
            .into_iter()
            .find(|agent| agent.id == id)
            .ok_or_else(|| AppError::NotFound {
                entity: crate::app::EntityKind::Agent,
                id: id.to_string(),
            });
    }
    let matches: Vec<_> = agents
        .into_iter()
        .filter(|agent| !agent.archived && agent.name.eq_ignore_ascii_case(raw.trim()))
        .collect();
    match matches.as_slice() {
        [] => Err(AppError::Precondition {
            problem: format!("no active agent named `{raw}` exists."),
            hint: "Check the agent name or use its identifier.".to_owned(),
        }),
        [agent] => Ok(agent.clone()),
        _ => Err(AppError::Precondition {
            problem: format!("more than one active agent is named `{raw}`."),
            hint: "Use the agent identifier instead of the name.".to_owned(),
        }),
    }
}

fn map_unique(error: UniqueIdError<StorageError>) -> AppError {
    match error {
        UniqueIdError::Lookup(storage) => AppError::from(storage),
        UniqueIdError::Exhausted => AppError::Internal,
    }
}
