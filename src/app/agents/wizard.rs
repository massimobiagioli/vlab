//! The guided `--new-agent` procedure (specification §7.2, A-09, A-22).
//!
//! The wizard is a use case: it asks through the [`Prompter`] port, validates
//! every answer, checks the one-Tech-Lead rule before writing, shows a
//! summary, and inserts the profile and all memberships in one transaction.
//! It never calls a provider and never executes user or model text.

use crate::app::AppError;
use crate::app::directory::Directory;
use crate::app::storage::AppStateStore;
use crate::domain::agent::{AgentProfile, Description, Role};
use crate::domain::ids::{AgentId, LabId, UniqueIdError, unique_short_id};
use crate::domain::lab::ensure_can_join;
use crate::domain::name::Name;
use crate::domain::settings::{ModelId, ProviderId, Settings};
use crate::domain::skills::{self, SkillTag};

/// Maximum invalid answers accepted for one prompt before aborting.
const MAX_PROMPT_ATTEMPTS: usize = 10;

/// One question. `default` is shown in brackets and used when the answer is
/// empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prompt<'a> {
    pub label: &'a str,
    pub default: Option<&'a str>,
}

/// The interactive surface used by the wizard. Production uses the terminal;
/// tests use a scripted implementation.
pub trait Prompter {
    /// False when stdin is not a terminal. The wizard refuses to run (A-09).
    fn is_interactive(&self) -> bool {
        true
    }

    /// Returns the raw answer, without the trailing newline.
    fn ask(&mut self, prompt: &Prompt<'_>) -> Result<String, AppError>;

    /// Confirms with a `y/N` question. EOF is treated as cancellation.
    fn confirm(&mut self, question: &str) -> Result<bool, AppError>;

    /// Shows a validation message and asks again.
    fn show_error(&mut self, _message: &str) {}
}

/// Runs the wizard and persists the agent. A declined summary returns
/// [`AppError::Cancelled`] and writes nothing.
/// Refuses a non-interactive invocation (A-09). Called before any database or
/// settings work so a piped invocation creates nothing and never waits.
pub fn ensure_interactive(prompter: &impl Prompter) -> Result<(), AppError> {
    if prompter.is_interactive() {
        Ok(())
    } else {
        Err(AppError::Precondition {
            problem: "`--new-agent` requires an interactive terminal.".to_owned(),
            hint:
                "Run `vlab --new-agent` from a terminal; non-interactive creation is not supported."
                    .to_owned(),
        })
    }
}

pub fn run(
    prompter: &mut impl Prompter,
    store: &impl Directory,
    state: &impl AppStateStore,
    settings: &Settings,
) -> Result<AgentProfile, AppError> {
    ensure_interactive(prompter)?;

    let name = ask_valid(prompter, "Name", None, |answer| {
        Name::parse(answer).map_err(|error| error.to_string())
    })?;
    let role = ask_valid(prompter, "Role (lead, dev, analyst, qa)", None, |answer| {
        Role::parse(answer).map_err(|error| error.to_string())
    })?;
    let description = ask_valid(prompter, "Description", None, |answer| {
        Description::parse(answer).map_err(|error| error.to_string())
    })?;
    let skills = ask_skills(prompter, role)?;

    let provider = ask_valid(
        prompter,
        "Provider",
        Some(settings.provider.as_str()),
        |answer| ProviderId::parse(answer).map_err(|error| error.to_string()),
    )?;
    let model = ask_valid(prompter, "Model", Some(settings.model.as_str()), |answer| {
        ModelId::parse(answer).map_err(|error| error.to_string())
    })?;

    let cv = ask_cv(prompter)?;
    let memberships = ask_memberships(prompter, store, state)?;

    // Validate the one-Tech-Lead rule for every target laboratory before the
    // summary, so a rejected choice cannot look like a save.
    for lab in &memberships {
        let existing = store.lab_agents(lab)?;
        let roles = existing
            .iter()
            .filter(|member| !member.archived)
            .filter_map(|member| member.role().ok());
        ensure_can_join(roles, role).map_err(|error| AppError::Validation {
            problem: format!("{error} Laboratory: {lab}."),
            hint: "Choose a different role for this agent or edit the existing Tech Lead."
                .to_owned(),
        })?;
    }

    let summary = summarize(
        &name,
        role,
        &description,
        &skills,
        &provider,
        &model,
        cv.as_deref(),
        &memberships,
    );
    if !prompter.confirm(&format!("{summary}\nSave this agent? [y/N]"))? {
        return Err(AppError::Cancelled);
    }

    let generated = unique_short_id(|candidate| match AgentId::parse(candidate) {
        Some(id) => store.agent_exists(&id),
        None => Ok(true),
    })
    .map_err(map_unique)?;
    let id = AgentId::parse(&generated).ok_or(AppError::Internal)?;
    let profile = AgentProfile::new(id, name, role, description, skills, provider, model, cv)
        .map_err(|error| AppError::Validation {
            problem: error.to_string(),
            hint: "Run `vlab --new-agent` again and correct the profile.".to_owned(),
        })?;
    store.insert_agent(&profile, &memberships)?;
    Ok(profile)
}

fn ask_valid<T>(
    prompter: &mut impl Prompter,
    label: &str,
    default: Option<&str>,
    parse: impl Fn(&str) -> Result<T, String>,
) -> Result<T, AppError> {
    for _ in 0..MAX_PROMPT_ATTEMPTS {
        let answer = prompter.ask(&Prompt { label, default })?;
        let answer = if answer.trim().is_empty() {
            default.unwrap_or("").to_owned()
        } else {
            answer
        };
        match parse(&answer) {
            Ok(value) => return Ok(value),
            Err(reason) => prompter.show_error(&reason),
        }
    }
    Err(AppError::Validation {
        problem: "too many invalid answers.".to_owned(),
        hint: "Run `vlab --new-agent` again when you have the profile ready.".to_owned(),
    })
}

fn ask_skills(prompter: &mut impl Prompter, role: Role) -> Result<Vec<SkillTag>, AppError> {
    for _ in 0..MAX_PROMPT_ATTEMPTS {
        let answer = prompter.ask(&Prompt {
            label: "Skills (comma-separated tags, for example rust, api, qa)",
            default: None,
        })?;
        match skills::parse_list(&answer) {
            Ok(tags) if role.requires_skills() && tags.is_empty() => {
                prompter.show_error("this role needs at least one skill.");
            }
            Ok(tags) => return Ok(tags),
            Err(error) => prompter.show_error(&error.to_string()),
        }
    }
    Err(AppError::Validation {
        problem: "too many invalid answers.".to_owned(),
        hint: "Run `vlab --new-agent` again when you have the profile ready.".to_owned(),
    })
}

fn ask_cv(prompter: &mut impl Prompter) -> Result<Option<String>, AppError> {
    let answer = prompter.ask(&Prompt {
        label: "CV or professional profile (optional, blank to skip)",
        default: None,
    })?;
    if answer.trim().is_empty() {
        return Ok(None);
    }
    if answer.len() > crate::domain::agent::CV_MAX_BYTES {
        return Err(AppError::Validation {
            problem: "the CV is larger than 64 KiB.".to_owned(),
            hint: "Shorten the text and run `vlab --new-agent` again.".to_owned(),
        });
    }
    if answer.contains('\0') {
        return Err(AppError::Validation {
            problem: "the CV contains an invalid character.".to_owned(),
            hint: "Remove NUL characters and run `vlab --new-agent` again.".to_owned(),
        });
    }
    Ok(Some(answer))
}

fn ask_memberships(
    prompter: &mut impl Prompter,
    store: &impl Directory,
    state: &impl AppStateStore,
) -> Result<Vec<LabId>, AppError> {
    let active = crate::app::context::active_lab(state)?;
    let mut defaults = Vec::new();
    if let crate::app::context::ActiveLab::Selected(id) = active {
        // A stale selection can survive an out-of-band deletion; only a
        // laboratory that still exists is offered as the pre-filled default.
        if store.lab_exists(&id)? {
            defaults.push(id);
        }
    }
    if defaults.is_empty() && store.labs()?.is_empty() {
        return Err(AppError::Precondition {
            problem: "no laboratory exists to join.".to_owned(),
            hint: "Run `vlab --new-lab \"Name\"` first, then create the agent again.".to_owned(),
        });
    }
    let label = if defaults.is_empty() {
        "Laboratory IDs (comma-separated, at least one)"
    } else {
        "Additional laboratory IDs (comma-separated, blank for none)"
    };

    for _ in 0..MAX_PROMPT_ATTEMPTS {
        let answer = prompter.ask(&Prompt {
            label,
            default: None,
        })?;
        let mut labs = defaults.clone();
        let mut invalid = None;
        for token in answer.split(|c: char| c == ',' || c.is_whitespace()) {
            let token = token.trim();
            if token.is_empty() {
                continue;
            }
            match LabId::parse(token) {
                Some(id) => {
                    if !store.lab_exists(&id)? {
                        invalid = Some(format!("no laboratory `{id}` exists."));
                        break;
                    }
                    if !labs.contains(&id) {
                        labs.push(id);
                    }
                }
                None => {
                    invalid = Some(
                        "laboratory identifiers are 8 lowercase hexadecimal characters.".to_owned(),
                    );
                    break;
                }
            }
        }
        if let Some(reason) = invalid {
            prompter.show_error(&reason);
            continue;
        }
        if labs.is_empty() {
            prompter.show_error("at least one laboratory is required.");
            continue;
        }
        return Ok(labs);
    }
    Err(AppError::Validation {
        problem: "too many invalid answers.".to_owned(),
        hint: "Run `vlab --new-agent` again when you have the laboratory IDs ready.".to_owned(),
    })
}

#[allow(clippy::too_many_arguments)]
fn summarize(
    name: &Name,
    role: Role,
    description: &Description,
    skills: &[SkillTag],
    provider: &ProviderId,
    model: &ModelId,
    cv: Option<&str>,
    memberships: &[LabId],
) -> String {
    let skill_list = if skills.is_empty() {
        "none".to_owned()
    } else {
        skills
            .iter()
            .map(SkillTag::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let labs = memberships
        .iter()
        .map(LabId::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Name: {name}\nRole: {}\nDescription: {}\nSkills: {skill_list}\nProvider: {provider}\nModel: {model}\nCV: {}\nLaboratories: {labs}",
        role.as_str(),
        description.as_str(),
        if cv.is_some() { "provided" } else { "none" },
    )
}

fn map_unique(error: UniqueIdError<crate::app::storage::StorageError>) -> AppError {
    match error {
        UniqueIdError::Lookup(storage) => AppError::from(storage),
        UniqueIdError::Exhausted => AppError::Internal,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::{HashMap, VecDeque};

    use super::*;
    use crate::app::directory::{AgentRecord, LabOverview};
    use crate::app::storage::{AppStateKey, StorageError};
    use crate::domain::lab::Lab;

    struct Scripted {
        answers: VecDeque<String>,
        confirm: VecDeque<bool>,
        errors: RefCell<Vec<String>>,
        interactive: bool,
    }

    impl Scripted {
        fn new(answers: &[&str], confirm: bool) -> Self {
            Self {
                answers: answers.iter().map(|a| (*a).to_owned()).collect(),
                confirm: VecDeque::from([confirm]),
                errors: RefCell::new(Vec::new()),
                interactive: true,
            }
        }

        fn non_interactive(answers: &[&str]) -> Self {
            Self {
                interactive: false,
                ..Self::new(answers, false)
            }
        }
    }

    impl Prompter for Scripted {
        fn is_interactive(&self) -> bool {
            self.interactive
        }

        fn ask(&mut self, _: &Prompt<'_>) -> Result<String, AppError> {
            self.answers.pop_front().ok_or(AppError::Cancelled)
        }

        fn confirm(&mut self, _: &str) -> Result<bool, AppError> {
            self.confirm.pop_front().ok_or(AppError::Cancelled)
        }

        fn show_error(&mut self, message: &str) {
            self.errors.borrow_mut().push(message.to_owned());
        }
    }

    #[derive(Default)]
    struct Store {
        labs: RefCell<HashMap<LabId, LabOverview>>,
        agents: RefCell<HashMap<AgentId, AgentRecord>>,
        memberships: RefCell<HashMap<AgentId, Vec<LabId>>>,
        inserts: RefCell<Vec<(AgentProfile, Vec<LabId>)>>,
    }

    impl Directory for Store {
        fn insert_lab(&self, _: &Lab) -> Result<(), StorageError> {
            unreachable!()
        }
        fn lab_exists(&self, id: &LabId) -> Result<bool, StorageError> {
            Ok(self.labs.borrow().contains_key(id))
        }
        fn lab(&self, id: &LabId) -> Result<Option<LabOverview>, StorageError> {
            Ok(self.labs.borrow().get(id).cloned())
        }
        fn labs(&self) -> Result<Vec<LabOverview>, StorageError> {
            Ok(self.labs.borrow().values().cloned().collect())
        }
        fn lab_agents(&self, lab: &LabId) -> Result<Vec<AgentRecord>, StorageError> {
            Ok(self
                .memberships
                .borrow()
                .iter()
                .filter(|(_, labs)| labs.contains(lab))
                .filter_map(|(id, _)| self.agents.borrow().get(id).cloned())
                .collect())
        }
        fn insert_agent(
            &self,
            profile: &AgentProfile,
            memberships: &[LabId],
        ) -> Result<(), StorageError> {
            self.inserts
                .borrow_mut()
                .push((profile.clone(), memberships.to_vec()));
            Ok(())
        }
        fn agent_exists(&self, id: &AgentId) -> Result<bool, StorageError> {
            Ok(self.agents.borrow().contains_key(id))
        }
        fn agent(&self, id: &AgentId) -> Result<Option<AgentRecord>, StorageError> {
            Ok(self.agents.borrow().get(id).cloned())
        }
        fn agent_memberships(&self, _: &AgentId) -> Result<Vec<LabOverview>, StorageError> {
            Ok(Vec::new())
        }
    }

    #[derive(Default)]
    struct State(RefCell<Option<String>>);

    impl AppStateStore for State {
        fn get(&self, _: AppStateKey) -> Result<Option<String>, StorageError> {
            Ok(self.0.borrow().clone())
        }
        fn set(&self, _: AppStateKey, value: &str) -> Result<(), StorageError> {
            *self.0.borrow_mut() = Some(value.to_owned());
            Ok(())
        }
        fn clear(&self, _: AppStateKey) -> Result<(), StorageError> {
            *self.0.borrow_mut() = None;
            Ok(())
        }
    }

    fn settings() -> Settings {
        Settings::parse("deepseek", "deepseek-chat").unwrap()
    }

    fn lab(id: &str) -> LabOverview {
        LabOverview {
            id: LabId::parse(id).unwrap(),
            name: "Lab".to_owned(),
            agent_count: 0,
            project_count: 0,
        }
    }

    fn state() -> State {
        let state = State::default();
        *state.0.borrow_mut() = Some("a3f91c07".to_owned());
        state
    }

    #[test]
    fn the_happy_path_persists_profile_and_memberships_together() {
        let store = Store::default();
        store
            .labs
            .borrow_mut()
            .insert(LabId::parse("a3f91c07").unwrap(), lab("a3f91c07"));
        let mut prompter = Scripted::new(
            &["Ada", "lead", "Coordinates planning", "", "", "", "", ""],
            true,
        );
        let profile = run(&mut prompter, &store, &state(), &settings()).unwrap();
        assert_eq!(profile.name.as_str(), "Ada");
        assert_eq!(profile.role, Role::Lead);
        assert_eq!(profile.provider, ProviderId::DeepSeek);
        assert_eq!(profile.model.as_str(), "deepseek-chat");
        let inserts = store.inserts.borrow();
        assert_eq!(inserts.len(), 1);
        assert_eq!(inserts[0].0, profile);
        assert_eq!(inserts[0].1, vec![LabId::parse("a3f91c07").unwrap()]);
    }

    #[test]
    fn invalid_answers_are_reported_and_reprompted() {
        let store = Store::default();
        store
            .labs
            .borrow_mut()
            .insert(LabId::parse("a3f91c07").unwrap(), lab("a3f91c07"));
        let mut prompter = Scripted::new(
            &[
                "",
                "Ada",
                "boss",
                "dev",
                "Writes code",
                "not a skill",
                "rust",
                "openai",
                "deepseek",
                "two words",
                "deepseek-chat",
                "",
                "",
            ],
            true,
        );
        let profile = run(&mut prompter, &store, &state(), &settings()).unwrap();
        assert_eq!(profile.role, Role::Dev);
        assert_eq!(profile.skills.len(), 1);
        assert!(prompter.errors.borrow().len() >= 3, "{:?}", prompter.errors);
    }

    #[test]
    fn a_second_lead_is_rejected_before_any_write() {
        let store = Store::default();
        let lab_id = LabId::parse("a3f91c07").unwrap();
        store
            .labs
            .borrow_mut()
            .insert(lab_id.clone(), lab("a3f91c07"));
        let lead = AgentRecord {
            id: AgentId::parse("0b1c2d3e").unwrap(),
            name: "Lead".to_owned(),
            role: "lead".to_owned(),
            description: "Leads".to_owned(),
            skills_json: "[]".to_owned(),
            provider: "deepseek".to_owned(),
            model: "deepseek-chat".to_owned(),
            cv: None,
            archived: false,
        };
        store.agents.borrow_mut().insert(lead.id.clone(), lead);
        store
            .memberships
            .borrow_mut()
            .insert(AgentId::parse("0b1c2d3e").unwrap(), vec![lab_id]);

        let mut prompter = Scripted::new(
            &["Ada", "lead", "Coordinates planning", "", "", "", "", ""],
            true,
        );
        let error = run(&mut prompter, &store, &state(), &settings()).unwrap_err();
        assert!(matches!(error, AppError::Validation { .. }), "{error:?}");
        assert!(store.inserts.borrow().is_empty());
    }

    #[test]
    fn unknown_laboratories_are_reported_and_reprompted() {
        let store = Store::default();
        store
            .labs
            .borrow_mut()
            .insert(LabId::parse("a3f91c07").unwrap(), lab("a3f91c07"));
        let mut prompter = Scripted::new(
            &["Ada", "qa", "Tests", "qa", "", "", "", "ffffffff", ""],
            true,
        );
        let profile = run(&mut prompter, &store, &state(), &settings()).unwrap();
        assert_eq!(profile.role, Role::Qa);
        assert!(
            prompter
                .errors
                .borrow()
                .iter()
                .any(|e| e.contains("no laboratory"))
        );
    }

    #[test]
    fn a_declined_summary_cancels_without_writing() {
        let store = Store::default();
        store
            .labs
            .borrow_mut()
            .insert(LabId::parse("a3f91c07").unwrap(), lab("a3f91c07"));
        let mut prompter = Scripted::new(
            &["Ada", "lead", "Coordinates planning", "", "", "", "", ""],
            false,
        );
        let error = run(&mut prompter, &store, &state(), &settings()).unwrap_err();
        assert!(matches!(error, AppError::Cancelled), "{error:?}");
        assert!(store.inserts.borrow().is_empty());
    }

    #[test]
    fn a_missing_laboratory_is_reported_before_the_membership_prompt() {
        let store = Store::default();
        let mut prompter = Scripted::new(&["Ada", "lead", "Leads", "", "", "", ""], true);
        let error = run(&mut prompter, &store, &state(), &settings()).unwrap_err();
        assert!(matches!(error, AppError::Precondition { .. }), "{error:?}");
        assert!(store.inserts.borrow().is_empty());
    }

    #[test]
    fn a_stale_active_selection_is_not_used_as_a_default() {
        let store = Store::default();
        store
            .labs
            .borrow_mut()
            .insert(LabId::parse("1c2d3e4f").unwrap(), lab("1c2d3e4f"));
        // `state()` selects a3f91c07, which does not exist.
        let mut prompter =
            Scripted::new(&["Ada", "lead", "Leads", "", "", "", "", "1c2d3e4f"], true);
        let profile = run(&mut prompter, &store, &state(), &settings()).unwrap();
        assert_eq!(profile.role, Role::Lead);
        assert_eq!(
            store.inserts.borrow()[0].1,
            vec![LabId::parse("1c2d3e4f").unwrap()]
        );
    }

    #[test]
    fn a_non_interactive_invocation_is_refused_before_any_work() {
        let store = Store::default();
        let mut prompter = Scripted::non_interactive(&[]);
        let error = run(&mut prompter, &store, &state(), &settings()).unwrap_err();
        assert!(matches!(error, AppError::Precondition { .. }), "{error:?}");
        assert!(store.inserts.borrow().is_empty());
    }
}
