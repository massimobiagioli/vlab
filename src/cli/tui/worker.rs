//! Serial in-process Tech Lead worker (ADR-0061).
//!
//! The worker owns its SQLite connection and never touches the terminal. It
//! selects the oldest `TODO` request for the active laboratory, runs the normal
//! bounded planning and workflow, and reports one event per transition.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use crate::app::context::{self, ActiveLab};
use crate::app::planning::PlanningStore;
use crate::app::queue;
use crate::app::requests::{self, PlanningOutcome};
use crate::app::settings;
use crate::app::workflow;
use crate::domain::budget::{CancelToken, ExecutionLimits};
use crate::domain::ids::RequestId;
use crate::infra::config::SettingsFile;
use crate::infra::db::{DEFAULT_BUSY_TIMEOUT, Database};
use crate::infra::execution::WorkspaceEnvironment;
use crate::infra::paths::Paths;
use crate::obs::RunId;

const IDLE_POLL: Duration = Duration::from_millis(250);
const ERROR_BACKOFF: Duration = Duration::from_secs(5);

/// Bounded workflow counters reported after a queued request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WorkflowSummary {
    pub completed: usize,
    pub failed: usize,
    pub remaining: usize,
    pub remediated: usize,
    pub debt: usize,
}

/// One observable transition from the worker.
pub(super) enum WorkerEvent {
    Started {
        request_id: RequestId,
    },
    Finished {
        request_id: RequestId,
        outcome: PlanningOutcome,
        workflow: Option<Result<WorkflowSummary, String>>,
    },
    Background(String),
    Warning(String),
}

/// A running worker plus the receiver the dashboard polls.
pub(super) struct WorkerHandle {
    shutdown: Arc<AtomicBool>,
    cancel: CancelToken,
    pub(super) events: Receiver<WorkerEvent>,
}

impl WorkerHandle {
    pub(super) fn stop(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.cancel.cancel();
    }
}

/// Starts the worker once. A failed spawn leaves the dashboard functional.
pub(super) fn spawn(paths: Paths, run_id: RunId) -> WorkerHandle {
    let shutdown = Arc::new(AtomicBool::new(false));
    let cancel = CancelToken::default();
    let (sender, events) = mpsc::channel();
    let thread_shutdown = Arc::clone(&shutdown);
    let thread_cancel = cancel.clone();
    let _ = thread::Builder::new()
        .name("vlab-queue".to_owned())
        .spawn(move || run(paths, run_id, thread_shutdown, thread_cancel, sender));
    WorkerHandle {
        shutdown,
        cancel,
        events,
    }
}

fn run(
    paths: Paths,
    run_id: RunId,
    shutdown: Arc<AtomicBool>,
    cancel: CancelToken,
    tx: Sender<WorkerEvent>,
) {
    let db = match Database::open_with_data_dir(
        paths.database_file(),
        paths.data_dir(),
        DEFAULT_BUSY_TIMEOUT,
    ) {
        Ok(db) => db,
        Err(error) => {
            let _ = tx.send(WorkerEvent::Warning(format!(
                "the background queue cannot open storage: {error}"
            )));
            return;
        }
    };
    let _ = tx.send(WorkerEvent::Background(
        "Background: checking schedules and pending work...".to_owned(),
    ));
    let mut startup = Vec::new();
    crate::cli::dispatch::resume_pending_workflow(&mut startup, run_id, cancel.clone());
    for line in String::from_utf8_lossy(&startup)
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let _ = tx.send(WorkerEvent::Background(line.to_owned()));
    }
    let _ = tx.send(WorkerEvent::Background(
        "Background: startup check complete.".to_owned(),
    ));
    let settings_file = SettingsFile::new(paths.config_dir(), paths.settings_file());
    let mut last_warning = String::new();
    while !shutdown.load(Ordering::SeqCst) && !cancel.is_cancelled() {
        let lab = match context::active_lab(&db) {
            Ok(ActiveLab::Selected(lab)) => lab,
            _ => {
                thread::sleep(IDLE_POLL);
                continue;
            }
        };
        let request = match queue::next(&db, &lab) {
            Ok(Some(request)) => request,
            Ok(None) => {
                thread::sleep(IDLE_POLL);
                continue;
            }
            Err(error) => {
                warn_once(
                    &tx,
                    &mut last_warning,
                    format!("queue read failed: {error}"),
                );
                thread::sleep(ERROR_BACKOFF);
                continue;
            }
        };
        let settings = match settings::show(&settings_file) {
            Ok(report) => report.settings,
            Err(error) => {
                warn_once(
                    &tx,
                    &mut last_warning,
                    format!("queue paused: {} {}", error, error.hint()),
                );
                thread::sleep(ERROR_BACKOFF);
                continue;
            }
        };
        let limits = ExecutionLimits::default();
        let provider = match crate::cli::dispatch::provider_from_settings(&settings, limits) {
            Ok(provider) => provider,
            Err(error) => {
                warn_once(
                    &tx,
                    &mut last_warning,
                    format!("queue paused: {} {}", error, error.hint()),
                );
                thread::sleep(ERROR_BACKOFF);
                continue;
            }
        };
        last_warning.clear();
        let _ = tx.send(WorkerEvent::Started {
            request_id: request.id.clone(),
        });
        let environment = WorkspaceEnvironment::new(paths.workspace_dir());
        let outcome = requests::plan_queued(
            &db,
            &db,
            &request,
            &settings,
            &provider,
            limits,
            &crate::domain::clock::SystemClock,
            cancel.clone(),
            run_id,
        );
        if let Err(error) = &outcome {
            let _ = PlanningStore::finish_failed(
                &db,
                &request.id,
                "queue_precondition",
                "the queued request could not be planned.",
            );
            warn_once(
                &tx,
                &mut last_warning,
                format!("request {} failed: {} {}", request.id, error, error.hint()),
            );
        }
        let workflow = match &outcome {
            Ok(PlanningOutcome::Planned { .. }) => Some(
                workflow::run(
                    &db,
                    &db,
                    &request.project_id,
                    &settings,
                    &provider,
                    &environment,
                    limits,
                    &crate::domain::clock::SystemClock,
                    cancel.clone(),
                    run_id,
                )
                .map(|report| WorkflowSummary {
                    completed: report.completed,
                    failed: report.failed,
                    remaining: report.remaining,
                    remediated: report.remediated,
                    debt: report.debt,
                })
                .map_err(|error| format!("{} {}", error, error.hint())),
            ),
            _ => None,
        };
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(_) => PlanningOutcome::Failed {
                request_id: request.id.clone(),
                code: "queue_precondition",
                hint: "the queued request could not be planned.",
                validation: true,
            },
        };
        let _ = tx.send(WorkerEvent::Finished {
            request_id: request.id,
            outcome,
            workflow,
        });
    }
}

fn warn_once(tx: &Sender<WorkerEvent>, last: &mut String, message: String) {
    if *last != message {
        last.clone_from(&message);
        let _ = tx.send(WorkerEvent::Warning(message));
    }
}
