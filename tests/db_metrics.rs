//! Laboratory metrics aggregate (ADR-0051).

mod support;

use vlab::app::context::save_active_lab;
use vlab::app::metrics;
use vlab::domain::ids::LabId;
use vlab::infra::db::Database;
use vlab::obs::RunId;

fn seed() -> (tempfile::TempDir, Database, LabId) {
    let dir = support::private_tempdir();
    let db = Database::open(dir.path().join("vlab.db")).unwrap();
    db.migrate(RunId::generate()).unwrap();
    let lab = LabId::parse("a3f91c07").unwrap();
    db.execute(
        "INSERT INTO labs (id, name, created_at, updated_at) \
         VALUES ('a3f91c07', 'Payments', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    save_active_lab(&db, &lab).unwrap();
    db.execute(
        "INSERT INTO projects (id, lab_id, name, created_at, updated_at) \
         VALUES ('0b1c2d3e', 'a3f91c07', 'Portal', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    db.execute(
        "INSERT INTO projects (id, lab_id, name, created_at, updated_at) \
         VALUES ('1c2d3e4f', 'a3f91c07', 'Empty', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    db.execute(
        "INSERT INTO requests (id, project_id, content, status, created_at, updated_at) \
         VALUES ('2d3e4f50', '0b1c2d3e', 'R', 'PLANNED', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    db.execute(
        "INSERT INTO requests (id, project_id, content, status, created_at, updated_at) \
         VALUES ('3e4f5061', '0b1c2d3e', 'R2', 'TODO', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    for sql in [
        "INSERT INTO stories (id, project_id, request_id, title, description, \
         acceptance_criteria_json, priority, complexity, required_skills_json, \
         state, due_date, created_at, updated_at) \
         VALUES ('4f506172', '0b1c2d3e', '2d3e4f50', 'Story', 'D', '[]', 'high', \
         'small', '[]', 'TODO', '2026-01-03', '2026-01-01T00:00:00Z', \
         '2026-01-01T00:00:00Z')",
        "INSERT INTO stories (id, project_id, request_id, title, description, \
         acceptance_criteria_json, priority, complexity, required_skills_json, \
         state, due_date, created_at, updated_at) \
         VALUES ('50617283', '0b1c2d3e', '2d3e4f50', 'Story', 'D', '[]', 'high', \
         'small', '[]', 'IN_PROGRESS', '2026-01-03', '2026-01-01T00:00:00Z', \
         '2026-01-01T00:00:00Z')",
        "INSERT INTO stories (id, project_id, request_id, title, description, \
         acceptance_criteria_json, priority, complexity, required_skills_json, \
         state, due_date, created_at, updated_at) \
         VALUES ('61728394', '0b1c2d3e', '2d3e4f50', 'Story', 'D', '[]', 'high', \
         'small', '[]', 'BLOCKED', '2026-01-03', '2026-01-01T00:00:00Z', \
         '2026-01-01T00:00:00Z')",
        "INSERT INTO stories (id, project_id, request_id, title, description, \
         acceptance_criteria_json, priority, complexity, required_skills_json, \
         state, due_date, created_at, updated_at) \
         VALUES ('728394a5', '0b1c2d3e', '2d3e4f50', 'Story', 'D', '[]', 'high', \
         'small', '[]', 'DONE', '2026-01-03', '2026-01-01T00:00:00Z', \
         '2026-01-01T00:00:00Z')",
        "INSERT INTO stories (id, project_id, request_id, title, description, \
         acceptance_criteria_json, priority, complexity, required_skills_json, \
         state, due_date, created_at, updated_at) \
         VALUES ('8394a5b6', '0b1c2d3e', '2d3e4f50', 'Story', 'D', '[]', 'high', \
         'small', '[]', 'DONE', '2026-01-03', '2026-01-01T00:00:00Z', \
         '2026-01-01T00:00:00Z')",
    ] {
        db.execute(sql).unwrap();
    }
    (dir, db, lab)
}

#[test]
fn lab_metrics_aggregate_states_and_projects() {
    let (_dir, db, lab) = seed();
    let metrics = metrics::show(&db, &lab).unwrap();
    assert_eq!(metrics.lab_name, "Payments");
    assert_eq!(metrics.projects.len(), 2);
    let counts = metrics.counts();
    assert_eq!(
        (
            counts.todo,
            counts.in_progress,
            counts.in_review,
            counts.blocked,
            counts.done
        ),
        (1, 1, 0, 1, 2)
    );
    assert_eq!(metrics.open(), 3);
    assert_eq!(metrics.closed(), 2);
    assert_eq!(metrics.total(), 5);
    assert_eq!(metrics.completion(), Some(40));
    assert_eq!(metrics.pending_intake, 1);
    let empty = metrics
        .projects
        .iter()
        .find(|project| project.name == "Empty")
        .unwrap();
    assert_eq!((empty.total(), empty.open(), empty.closed()), (0, 0, 0));
}

#[test]
fn an_unknown_laboratory_is_an_operation_error() {
    let (_dir, db, _lab) = seed();
    let missing = LabId::parse("ffffffff").unwrap();
    assert!(metrics::show(&db, &missing).is_err());
}
