//! Laboratory validation rules (specification §6.5, A-07).
//!
//! The rules are pure functions over one laboratory and its stored member
//! rows, so `--check-lab` and `--list-labs` share them. No provider call is
//! made or possible here.

use crate::app::directory::{AgentRecord, LabOverview};
use crate::domain::agent::Role;
use crate::domain::lab::lead_count;

/// One line of the structural report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleResult {
    pub rule: &'static str,
    pub passed: bool,
    /// A short, content-free explanation naming the affected agent IDs.
    pub detail: Option<String>,
}

impl RuleResult {
    pub fn pass(rule: &'static str, detail: impl Into<Option<String>>) -> Self {
        Self {
            rule,
            passed: true,
            detail: detail.into(),
        }
    }

    pub fn fail(rule: &'static str, detail: String) -> Self {
        Self {
            rule,
            passed: false,
            detail: Some(detail),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabCheck {
    pub lab: LabOverview,
    pub rules: Vec<RuleResult>,
}

impl LabCheck {
    pub fn valid(&self) -> bool {
        self.rules.iter().all(|rule| rule.passed)
    }

    pub fn failures(&self) -> impl Iterator<Item = &RuleResult> {
        self.rules.iter().filter(|rule| !rule.passed)
    }
}

/// Evaluates every §6.5 rule. A missing laboratory is handled by the caller,
/// which exits 3 before producing a report.
pub fn check(lab: &LabOverview, members: &[AgentRecord]) -> LabCheck {
    let active: Vec<&AgentRecord> = members.iter().filter(|member| !member.archived).collect();
    let mut rules = Vec::with_capacity(8);

    rules.push(RuleResult::pass("laboratory exists", None));

    rules.push(if active.is_empty() {
        RuleResult::fail(
            "at least one agent",
            "the laboratory has no active members".to_owned(),
        )
    } else {
        RuleResult::pass("at least one agent", format!("{} active", active.len()))
    });

    let parsed_roles: Vec<Option<Role>> = active.iter().map(|member| member.role().ok()).collect();
    let leads = lead_count(parsed_roles.iter().flatten().copied());
    rules.push(if leads == 1 {
        RuleResult::pass("exactly one Tech Lead", None)
    } else {
        RuleResult::fail(
            "exactly one Tech Lead",
            format!("found {leads}; exactly one member must have role `lead`"),
        )
    });

    let invalid_roles: Vec<&str> = active
        .iter()
        .zip(&parsed_roles)
        .filter(|(_, role)| role.is_none())
        .map(|(member, _)| member.id.as_str())
        .collect();
    rules.push(if invalid_roles.is_empty() {
        RuleResult::pass("roles are valid", None)
    } else {
        RuleResult::fail(
            "roles are valid",
            format!(
                "agents with an unsupported role: {}",
                invalid_roles.join(", ")
            ),
        )
    });

    let malformed: Vec<&str> = active
        .iter()
        .filter(|member| member.profile().is_err())
        .map(|member| member.id.as_str())
        .collect();
    rules.push(if malformed.is_empty() {
        RuleResult::pass("profiles are well-formed", None)
    } else {
        RuleResult::fail(
            "profiles are well-formed",
            format!(
                "agents with malformed stored fields: {}",
                malformed.join(", ")
            ),
        )
    });

    let bad_provider_model: Vec<&str> = active
        .iter()
        .filter(|member| {
            crate::domain::settings::ProviderId::parse(&member.provider).is_err()
                || crate::domain::settings::ModelId::parse(&member.model).is_err()
        })
        .map(|member| member.id.as_str())
        .collect();
    rules.push(if bad_provider_model.is_empty() {
        RuleResult::pass("provider and model are valid", None)
    } else {
        RuleResult::fail(
            "provider and model are valid",
            format!(
                "agents with an unsupported provider or model: {}",
                bad_provider_model.join(", ")
            ),
        )
    });

    let missing_skills: Vec<&str> = active
        .iter()
        .zip(&parsed_roles)
        .filter(|(member, role)| {
            role.is_some_and(|role| role.requires_skills())
                && member.skills().map(|tags| tags.is_empty()).unwrap_or(true)
        })
        .map(|(member, _)| member.id.as_str())
        .collect();
    rules.push(if missing_skills.is_empty() {
        RuleResult::pass("operational roles have skills", None)
    } else {
        RuleResult::fail(
            "operational roles have skills",
            format!(
                "agents with role dev, analyst, or qa and no skills: {}",
                missing_skills.join(", ")
            ),
        )
    });

    // Memberships are read through a join, so every row has a laboratory and
    // an agent. The rule is kept visible in the report because the
    // specification names it.
    rules.push(RuleResult::pass(
        "memberships resolve",
        format!("{} memberships", members.len()),
    ));

    LabCheck {
        lab: lab.clone(),
        rules,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::directory::AgentRecord;
    use crate::domain::agent::Role;
    use crate::domain::ids::{AgentId, LabId};
    use crate::domain::skills;

    fn lab() -> LabOverview {
        LabOverview {
            id: LabId::parse("a3f91c07").unwrap(),
            name: "Payments".to_owned(),
            description: String::new(),
            agent_count: 0,
            project_count: 0,
        }
    }

    fn member(role: Role, skills: &[&str]) -> AgentRecord {
        let tags = crate::domain::skills::parse_list(&skills.join(",")).unwrap();
        AgentRecord {
            id: AgentId::generate(),
            name: "Agent".to_owned(),
            role: role.as_str().to_owned(),
            description: "Does work".to_owned(),
            skills_json: skills::encode(&tags),
            provider: "deepseek".to_owned(),
            model: "deepseek-chat".to_owned(),
            cv: None,
            archived: false,
        }
    }

    fn rule<'a>(check: &'a LabCheck, name: &str) -> &'a RuleResult {
        check.rules.iter().find(|rule| rule.rule == name).unwrap()
    }

    #[test]
    fn a_complete_laboratory_passes_every_rule() {
        let members = vec![member(Role::Lead, &[]), member(Role::Dev, &["rust"])];
        let check = check(&lab(), &members);
        assert!(check.valid(), "{check:?}");
        assert_eq!(check.rules.len(), 8);
    }

    #[test]
    fn an_empty_laboratory_fails_the_membership_and_lead_rules() {
        let check = check(&lab(), &[]);
        assert!(!rule(&check, "at least one agent").passed);
        assert!(!rule(&check, "exactly one Tech Lead").passed);
    }

    #[test]
    fn two_leads_fail_only_the_lead_rule() {
        let members = vec![member(Role::Lead, &[]), member(Role::Lead, &[])];
        let check = check(&lab(), &members);
        assert!(!rule(&check, "exactly one Tech Lead").passed);
        assert!(rule(&check, "at least one agent").passed);
    }

    #[test]
    fn an_operational_member_without_skills_fails_the_skill_rule() {
        for role in [Role::Dev, Role::Analyst, Role::Qa] {
            let check = check(&lab(), &[member(Role::Lead, &[]), member(role, &[])]);
            assert!(
                !rule(&check, "operational roles have skills").passed,
                "{role:?}"
            );
        }
        let with_skills = check(
            &lab(),
            &[member(Role::Lead, &[]), member(Role::Dev, &["rust"])],
        );
        assert!(rule(&with_skills, "operational roles have skills").passed);
    }

    #[test]
    fn malformed_stored_fields_are_reported_without_panicking() {
        let mut broken = member(Role::Dev, &["rust"]);
        broken.role = "boss".to_owned();
        broken.provider = "openai".to_owned();
        broken.skills_json = "not-json".to_owned();
        let check = check(&lab(), &[member(Role::Lead, &[]), broken]);
        assert!(!rule(&check, "roles are valid").passed);
        assert!(!rule(&check, "profiles are well-formed").passed);
        assert!(!rule(&check, "provider and model are valid").passed);
    }

    #[test]
    fn archived_members_are_ignored_by_the_rules() {
        let mut archived_lead = member(Role::Lead, &[]);
        archived_lead.archived = true;
        let check = check(&lab(), &[archived_lead, member(Role::Lead, &[])]);
        assert!(check.valid(), "{check:?}");
    }
}
