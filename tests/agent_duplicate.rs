//! Agent duplication and optional memory preservation (ADR-0058).

mod support;

use support::Sandbox;
use vlab::app::agents;
use vlab::app::directory::Directory;
use vlab::app::memories::MemoryStore;
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::ids::{AgentId, LabId, MemoryId};
use vlab::domain::lab::Lab;
use vlab::domain::memory::{Memory, MemoryScope};
use vlab::domain::name::Name;
use vlab::domain::settings::{ModelId, ProviderId};
use vlab::domain::skills;
use vlab::infra::db::Database;

struct Fixture {
    _sandbox: Sandbox,
    db: Database,
    lab: LabId,
    lead: AgentId,
    dev: AgentId,
}

fn fixture() -> Fixture {
    let sandbox = Sandbox::new();
    let db = sandbox.seed_database();
    let lab = LabId::parse("a3f91c07").unwrap();
    db.insert_lab(&Lab::new(lab.clone(), Name::parse("Payments").unwrap()))
        .unwrap();
    let lead = AgentId::parse("10000001").unwrap();
    let dev = AgentId::parse("10000002").unwrap();
    for (id, name, role, skills_text) in [
        (&lead, "Klaus", Role::Lead, ""),
        (&dev, "Bram", Role::Dev, "rust, api"),
    ] {
        db.insert_agent(
            &AgentProfile::new(
                id.clone(),
                Name::parse(name).unwrap(),
                role,
                Description::parse("Works on the project").unwrap(),
                skills::parse_list(skills_text).unwrap(),
                ProviderId::DeepSeek,
                ModelId::parse("deepseek-chat").unwrap(),
                Some("Synthetic CV".to_owned()),
            )
            .unwrap(),
            std::slice::from_ref(&lab),
        )
        .unwrap();
    }
    let memory = Memory::new(
        MemoryId::parse("20000001").unwrap(),
        MemoryScope::Agent(dev.clone()),
        "Keep tests green.",
    )
    .unwrap();
    db.insert_memory(&memory).unwrap();
    Fixture {
        _sandbox: sandbox,
        db,
        lab,
        lead,
        dev,
    }
}

#[test]
fn duplicating_a_dev_copies_the_profile_memberships_and_memories() {
    let fixture = fixture();
    let report =
        agents::duplicate_agent(&fixture.db, &fixture.dev, "Bram copy", true, None).unwrap();
    assert_ne!(report.profile.id, fixture.dev);
    assert_eq!(report.profile.name.as_str(), "Bram copy");
    assert_eq!(report.profile.role, Role::Dev);
    assert_eq!(report.profile.description.as_str(), "Works on the project");
    assert_eq!(report.profile.skills.len(), 2);
    assert_eq!(report.profile.provider, ProviderId::DeepSeek);
    assert_eq!(report.profile.model.as_str(), "deepseek-chat");
    assert_eq!(report.profile.cv.as_deref(), Some("Synthetic CV"));
    assert_eq!(report.memberships, vec![fixture.lab.clone()]);
    assert_eq!(report.skipped_memberships, 0);
    assert_eq!(report.memories, 1);

    let stored = fixture.db.agent(&report.profile.id).unwrap().unwrap();
    let profile = stored.profile().unwrap();
    assert_eq!(profile.name.as_str(), "Bram copy");
    assert_eq!(
        fixture
            .db
            .agent_memberships(&report.profile.id)
            .unwrap()
            .iter()
            .map(|lab| lab.id.clone())
            .collect::<Vec<_>>(),
        vec![fixture.lab.clone()]
    );
    let memories = fixture
        .db
        .memories(&MemoryScope::Agent(report.profile.id.clone()))
        .unwrap();
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0].memory.content, "Keep tests green.");
    assert_ne!(memories[0].memory.id.as_str(), "20000001");
    // The source scope is unchanged.
    assert_eq!(
        fixture
            .db
            .memories(&MemoryScope::Agent(fixture.dev.clone()))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn a_duplicated_lead_gets_no_memberships_and_reports_the_skip() {
    let fixture = fixture();
    let report =
        agents::duplicate_agent(&fixture.db, &fixture.lead, "Lead copy", false, None).unwrap();
    assert_eq!(report.profile.role, Role::Lead);
    assert!(report.memberships.is_empty());
    assert_eq!(report.skipped_memberships, 1);
    assert_eq!(report.memories, 0);
    assert!(
        fixture
            .db
            .agent_memberships(&report.profile.id)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn memory_preservation_is_optional_and_validated() {
    let fixture = fixture();
    let without =
        agents::duplicate_agent(&fixture.db, &fixture.dev, "No memory copy", false, None).unwrap();
    assert_eq!(without.memories, 0);
    assert!(
        fixture
            .db
            .memories(&MemoryScope::Agent(without.profile.id.clone()))
            .unwrap()
            .is_empty()
    );

    let secret_memory = Memory::new(
        MemoryId::parse("20000002").unwrap(),
        MemoryScope::Agent(fixture.dev.clone()),
        "token sk-test-secret",
    )
    .unwrap();
    fixture.db.insert_memory(&secret_memory).unwrap();
    let before = fixture.db.all_agents().unwrap().len();
    let error = agents::duplicate_agent(
        &fixture.db,
        &fixture.dev,
        "Secret copy",
        true,
        Some("sk-test-secret"),
    )
    .unwrap_err();
    assert!(
        matches!(error, vlab::app::AppError::Validation { .. }),
        "{error:?}"
    );
    assert_eq!(fixture.db.all_agents().unwrap().len(), before);
}

#[test]
fn a_new_name_is_required_and_bounded() {
    let fixture = fixture();
    let error = agents::duplicate_agent(&fixture.db, &fixture.dev, "   ", false, None).unwrap_err();
    assert!(matches!(error, vlab::app::AppError::Validation { .. }));
    let error = agents::duplicate_agent(&fixture.db, &fixture.dev, &"x".repeat(101), false, None)
        .unwrap_err();
    assert!(matches!(error, vlab::app::AppError::Validation { .. }));
}

#[test]
fn duplicating_an_unknown_agent_is_missing() {
    let fixture = fixture();
    let missing = AgentId::parse("ffffffff").unwrap();
    let error = agents::duplicate_agent(&fixture.db, &missing, "Copy", false, None).unwrap_err();
    assert!(matches!(error, vlab::app::AppError::NotFound { .. }));
}
