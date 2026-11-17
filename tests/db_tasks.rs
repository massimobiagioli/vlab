//! Open task read model used by the laboratory floor (ADR-0062).

mod support;

use vlab::app::directory::Directory;
use vlab::app::kanban::StoryState;
use vlab::app::projects::ProjectStore;
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::ids::{AgentId, LabId, ProjectId};
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;
use vlab::domain::project::Project;
use vlab::domain::settings::{ModelId, ProviderId};
use vlab::domain::skills;
use vlab::infra::db::Database;

fn fixture() -> (tempfile::TempDir, Database, AgentId, ProjectId) {
    let dir = support::private_tempdir();
    let db = Database::open(dir.path().join("vlab.db")).unwrap();
    db.migrate(vlab::obs::RunId::generate()).unwrap();
    let lab = LabId::parse("a3f91c07").unwrap();
    db.insert_lab(&Lab::new(lab.clone(), Name::parse("Payments").unwrap()))
        .unwrap();
    let project = ProjectId::parse("0b1c2d3e").unwrap();
    db.insert_project(&Project::new(
        project.clone(),
        lab.clone(),
        Name::parse("Portal").unwrap(),
        None,
    ))
    .unwrap();
    let agent = AgentId::parse("aaaaaaaa").unwrap();
    db.insert_agent(
        &AgentProfile::new(
            agent.clone(),
            Name::parse("Grace").unwrap(),
            Role::Lead,
            Description::parse("Plans work").unwrap(),
            skills::parse_list("rust").unwrap(),
            ProviderId::DeepSeek,
            ModelId::parse("deepseek-chat").unwrap(),
            None,
        )
        .unwrap(),
        std::slice::from_ref(&lab),
    )
    .unwrap();
    // One request row keeps the story foreign key valid.
    db.execute(
        "INSERT INTO requests (id, project_id, content, status, created_at, updated_at) \
         VALUES ('1c2d3e4f', '0b1c2d3e', 'R', 'PLANNED', '2026-01-01T00:00:00Z', \
         '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    for (id, title, state, assignee) in [
        ("10000001", "Blocked work", "BLOCKED", Some("aaaaaaaa")),
        ("10000002", "Working", "IN_PROGRESS", Some("aaaaaaaa")),
        ("10000003", "Waiting", "TODO", None),
        ("10000004", "Finished", "DONE", Some("aaaaaaaa")),
    ] {
        let assigned = assignee.map_or("NULL".to_owned(), |value| format!("'{value}'"));
        db.execute(Box::leak(
            format!(
                "INSERT INTO stories (id, project_id, request_id, title, description, \
                 acceptance_criteria_json, priority, complexity, required_skills_json, \
                 assigned_agent_id, state, due_date, created_at, updated_at) \
                 VALUES ('{id}', '0b1c2d3e', '1c2d3e4f', '{title}', 'D', '[]', 'high', \
                 'small', '[]', {assigned}, '{state}', '2026-01-03', \
                 '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')"
            )
            .into_boxed_str(),
        ))
        .unwrap();
    }
    (dir, db, agent, project)
}

#[test]
fn project_and_agent_open_tasks_include_states_and_assignee() {
    let (_dir, db, agent, project) = fixture();
    let project_tasks = db.project_open_tasks(&project).unwrap();
    assert_eq!(
        project_tasks
            .iter()
            .map(|task| (task.title.as_str(), task.state))
            .collect::<Vec<_>>(),
        vec![
            ("Working", StoryState::InProgress),
            ("Blocked work", StoryState::Blocked),
            ("Waiting", StoryState::Todo),
        ]
    );
    assert!(
        project_tasks
            .iter()
            .all(|task| task.project_name == "Portal")
    );
    assert_eq!(
        project_tasks[0].assignee.as_ref(),
        Some(&agent),
        "the assigned task keeps the agent identifier"
    );
    assert_eq!(project_tasks[0].assignee_name.as_deref(), Some("Grace"));
    assert_eq!(
        db.agent_open_tasks(&agent)
            .unwrap()
            .iter()
            .map(|task| task.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Working", "Blocked work"]
    );
}
