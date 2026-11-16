//! Membership diff persistence for the populate screen (ADR-0053).

mod support;

use support::Sandbox;
use vlab::app::directory::Directory;
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::ids::{AgentId, LabId};
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;
use vlab::domain::settings::{ModelId, ProviderId};
use vlab::domain::skills;

fn seed(sandbox: &Sandbox) -> (LabId, AgentId, AgentId, AgentId, AgentId) {
    let db = sandbox.seed_database();
    let lab = LabId::parse("a3f91c07").unwrap();
    db.insert_lab(&Lab::new(lab.clone(), Name::parse("Payments").unwrap()))
        .unwrap();
    let lead = AgentId::parse("10000001").unwrap();
    let dev = AgentId::parse("10000002").unwrap();
    let qa = AgentId::parse("10000003").unwrap();
    let archived = AgentId::parse("10000004").unwrap();
    for (id, name, role, memberships) in [
        (&lead, "Klaus", Role::Lead, vec![lab.clone()]),
        (&dev, "Bram", Role::Dev, vec![lab.clone()]),
        (&qa, "Nigel", Role::Qa, Vec::new()),
        (&archived, "Old", Role::Dev, vec![lab.clone()]),
    ] {
        db.insert_agent(
            &AgentProfile::new(
                id.clone(),
                Name::parse(name).unwrap(),
                role,
                Description::parse("Works").unwrap(),
                skills::parse_list("rust").unwrap(),
                ProviderId::DeepSeek,
                ModelId::parse("deepseek-chat").unwrap(),
                None,
            )
            .unwrap(),
            &memberships,
        )
        .unwrap();
    }
    db.execute("UPDATE agents SET archived_at = '2026-01-01T00:00:00Z' WHERE id = '10000004'")
        .unwrap();
    db.execute(
        "UPDATE lab_memberships SET joined_at = 'SENTINEL' \
         WHERE lab_id = 'a3f91c07' AND agent_id = '10000001'",
    )
    .unwrap();
    (lab, lead, dev, qa, archived)
}

#[test]
fn the_diff_adds_and_removes_without_touching_unchanged_or_archived_members() {
    let sandbox = Sandbox::new();
    let (lab, lead, dev, qa, archived) = seed(&sandbox);
    let db = sandbox.seed_database();
    db.update_lab_memberships(&lab, std::slice::from_ref(&qa), std::slice::from_ref(&dev))
        .unwrap();

    let members = db.lab_agents(&lab).unwrap();
    let ids: Vec<&str> = members.iter().map(|record| record.id.as_str()).collect();
    assert!(ids.contains(&lead.as_str()), "{ids:?}");
    assert!(ids.contains(&qa.as_str()), "{ids:?}");
    assert!(!ids.contains(&dev.as_str()), "{ids:?}");
    assert!(
        ids.contains(&archived.as_str()),
        "archived membership was removed"
    );

    let joined: String = db
        .block_on(
            sqlx::query_scalar(
                "SELECT joined_at FROM lab_memberships WHERE lab_id = ?1 AND agent_id = ?2",
            )
            .bind(lab.as_str())
            .bind(lead.as_str())
            .fetch_one(db.pool()),
        )
        .unwrap();
    assert_eq!(joined, "SENTINEL");
}

#[test]
fn adding_an_existing_membership_is_idempotent() {
    let sandbox = Sandbox::new();
    let (lab, lead, _, _, _) = seed(&sandbox);
    let db = sandbox.seed_database();
    db.update_lab_memberships(&lab, std::slice::from_ref(&lead), &[])
        .unwrap();
    let members = db.lab_agents(&lab).unwrap();
    assert_eq!(members.iter().filter(|record| record.id == lead).count(), 1);
}
