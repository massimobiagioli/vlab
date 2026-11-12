//! Read-only Kanban behavior (WP4-AC3, WP4-AC4).

mod support;

use vlab::app::context::save_active_lab;
use vlab::app::directory::Directory;
use vlab::app::projects::ProjectStore;
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::ids::{AgentId, LabId, ProjectId};
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;
use vlab::domain::project::Project;
use vlab::domain::settings::{ModelId, ProviderId};
use vlab::domain::skills;

use support::Sandbox;

struct Fixture {
    project: ProjectId,
    lead: AgentId,
}

fn fixture(sandbox: &Sandbox) -> Fixture {
    let db = sandbox.seed_database();
    let lab = LabId::parse("a3f91c07").unwrap();
    db.insert_lab(&Lab::new(lab.clone(), Name::parse("Payments").unwrap()))
        .unwrap();
    save_active_lab(&db, &lab).unwrap();
    let project = ProjectId::parse("0b1c2d3e").unwrap();
    db.insert_project(&Project::new(
        project.clone(),
        lab.clone(),
        Name::parse("Portal").unwrap(),
        None,
    ))
    .unwrap();
    let lead = AgentId::parse("1c2d3e4f").unwrap();
    let profile = AgentProfile::new(
        lead.clone(),
        Name::parse("Ada").unwrap(),
        Role::Lead,
        Description::parse("Leads").unwrap(),
        skills::parse_list("").unwrap(),
        ProviderId::DeepSeek,
        ModelId::parse("deepseek-chat").unwrap(),
        None,
    )
    .unwrap();
    db.insert_agent(&profile, std::slice::from_ref(&lab))
        .unwrap();
    Fixture { project, lead }
}

fn exec(db: &vlab::infra::db::Database, sql: String) {
    db.block_on(async {
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .execute(db.pool())
            .await
            .unwrap();
    });
}

fn insert_request(db: &vlab::infra::db::Database, id: &str, status: &str) {
    exec(
        db,
        format!(
            "INSERT INTO requests (id, project_id, content, status, created_at, updated_at) \
             VALUES ('{id}', '0b1c2d3e', 'Request', '{status}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
        ),
    );
}

fn insert_story(
    db: &vlab::infra::db::Database,
    id: &str,
    title: &str,
    state: &str,
    assignee: Option<&str>,
) {
    let assignee = assignee
        .map(|a| format!("'{a}'"))
        .unwrap_or_else(|| "NULL".to_owned());
    exec(
        db,
        format!(
            "INSERT INTO stories (id, project_id, request_id, title, description, acceptance_criteria_json, priority, complexity, required_skills_json, assigned_agent_id, state, due_date, created_at, updated_at) \
             VALUES ('{id}', '0b1c2d3e', '2d3e4f50', '{title}', 'D', '[]', 'high', 'small', '[]', {assignee}, '{state}', '2026-01-03', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
        ),
    );
}

#[test]
fn an_empty_board_explains_what_to_expect() {
    let sandbox = Sandbox::new();
    let fixture = fixture(&sandbox);
    let outcome = sandbox.run(&["--show-project-kanban", fixture.project.as_str()]);
    outcome.assert_success().assert_no_escape_bytes();
    assert!(outcome.stdout.contains("Project 0b1c2d3e — Portal"));
    assert!(outcome.stdout.contains("Laboratory: Payments"));
    assert!(outcome.stdout.contains("Tech Lead: Ada"));
    assert!(outcome.stdout.contains("Stories: 0"));
    assert!(outcome.stdout.contains("Pending intake: 0"));
    assert!(outcome.stdout.contains("No stories yet."));
}

#[test]
fn the_board_shows_all_five_states_cards_and_pending_intake() {
    let sandbox = Sandbox::new();
    let fixture = fixture(&sandbox);
    let db = sandbox.seed_database();
    insert_request(&db, "2d3e4f50", "TODO");
    insert_request(&db, "3e4f5061", "PLANNED");
    insert_request(&db, "4f506172", "FAILED");
    insert_story(
        &db,
        "5a617283",
        "Add reset",
        "TODO",
        Some(fixture.lead.as_str()),
    );
    insert_story(&db, "6b728394", "Refactor", "IN_PROGRESS", None);
    insert_story(&db, "7c8394a5", "Review", "IN_REVIEW", None);
    insert_story(&db, "8d94a5b6", "Blocked", "BLOCKED", None);
    insert_story(&db, "9ea5b6c7", "Done", "DONE", None);

    let outcome = sandbox.run(&["--show-project-kanban", fixture.project.as_str()]);
    outcome.assert_success().assert_no_escape_bytes();
    for label in [
        "TODO (1)",
        "IN PROGRESS (1)",
        "IN REVIEW (1)",
        "BLOCKED (1)",
        "DONE (1)",
    ] {
        assert!(
            outcome.stdout.contains(label),
            "missing {label}:\n{}",
            outcome.stdout
        );
    }
    assert!(
        outcome.stdout.contains("Pending intake: 2"),
        "{}",
        outcome.stdout
    );
    assert!(outcome.stdout.contains("Add reset"), "{}", outcome.stdout);
    assert!(outcome.stdout.contains("Unassigned"), "{}", outcome.stdout);
    assert!(outcome.stdout.contains("Ada"), "{}", outcome.stdout);
    assert!(outcome.stdout.contains("2026-01-03"), "{}", outcome.stdout);
    assert!(outcome.stdout.contains("OVERDUE"), "{}", outcome.stdout);
}

#[test]
fn story_titles_are_sanitized() {
    let sandbox = Sandbox::new();
    let fixture = fixture(&sandbox);
    let db = sandbox.seed_database();
    insert_request(&db, "2d3e4f50", "TODO");
    insert_story(&db, "5a617283", "evil\u{1b}[31mtitle", "TODO", None);
    let outcome = sandbox.run(&["--show-project-kanban", fixture.project.as_str()]);
    outcome.assert_success().assert_no_escape_bytes();
    assert!(outcome.stdout.contains("eviltitle"), "{}", outcome.stdout);
}

#[test]
fn a_project_from_another_laboratory_is_not_found() {
    let sandbox = Sandbox::new();
    let fixture = fixture(&sandbox);
    let db = sandbox.seed_database();
    let other = LabId::parse("1c2d3e4f").unwrap();
    db.insert_lab(&Lab::new(other.clone(), Name::parse("Other").unwrap()))
        .unwrap();
    save_active_lab(&db, &other).unwrap();
    sandbox
        .run(&["--show-project-kanban", fixture.project.as_str()])
        .assert_failure(3, "no project with identifier");
}

#[test]
fn kanban_rendering_is_width_safe_in_unit_tests() {
    // The renderer itself is covered at widths 40/80/120 in `src/cli/render.rs`;
    // this test keeps the board's state labels tied to the public contract.
    let states = vlab::app::kanban::StoryState::ALL;
    let labels: Vec<&str> = states.iter().map(|state| state.label()).collect();
    assert_eq!(
        labels,
        ["TODO", "IN PROGRESS", "IN REVIEW", "BLOCKED", "DONE"]
    );
}
