//! Recurring project task schedules (ADR-0057).
//!
//! A schedule is a project-scoped task owned by one agent. When due, the
//! application creates a normal bounded request; the schedule never executes
//! model-chosen commands and has no background autonomy of its own.

use crate::domain::agent::Description;
use crate::domain::ids::{AgentId, ProjectId, RequestId, ScheduleId};
use crate::domain::name::Name;

/// Supported recurrence intervals. The initial release keeps the set small so
/// the page and the persisted value stay unambiguous.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleInterval {
    Daily,
    Weekly,
    Monthly,
}

impl ScheduleInterval {
    pub fn parse(input: &str) -> Result<Self, ScheduleIntervalError> {
        match input.trim().to_ascii_lowercase().as_str() {
            "daily" | "day" => Ok(Self::Daily),
            "weekly" | "week" => Ok(Self::Weekly),
            "monthly" | "month" => Ok(Self::Monthly),
            _ => Err(ScheduleIntervalError),
        }
    }

    pub fn from_seconds(seconds: i64) -> Option<Self> {
        match seconds {
            86_400 => Some(Self::Daily),
            604_800 => Some(Self::Weekly),
            2_592_000 => Some(Self::Monthly),
            _ => None,
        }
    }

    pub fn seconds(self) -> i64 {
        match self {
            Self::Daily => 86_400,
            Self::Weekly => 604_800,
            Self::Monthly => 2_592_000,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Daily => "Daily",
            Self::Weekly => "Weekly",
            Self::Monthly => "Monthly",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a schedule interval must be `daily`, `weekly`, or `monthly`.")]
pub struct ScheduleIntervalError;

/// One persisted schedule with its read-only last-request status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    pub id: ScheduleId,
    pub project_id: ProjectId,
    pub agent_id: AgentId,
    pub title: Name,
    pub request_text: Description,
    pub interval: ScheduleInterval,
    pub enabled: bool,
    pub next_run_at: String,
    pub last_run_at: Option<String>,
    pub last_request_id: Option<RequestId>,
    pub last_request_status: Option<String>,
    pub last_error_code: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intervals_round_trip_and_are_bounded() {
        for (label, seconds, expected) in [
            ("daily", 86_400, ScheduleInterval::Daily),
            (" WEEKLY ", 604_800, ScheduleInterval::Weekly),
            ("Monthly", 2_592_000, ScheduleInterval::Monthly),
        ] {
            let interval = ScheduleInterval::parse(label).unwrap();
            assert_eq!(interval, expected);
            assert_eq!(interval.seconds(), seconds);
            assert_eq!(ScheduleInterval::from_seconds(seconds), Some(expected));
        }
        assert_eq!(
            ScheduleInterval::parse("hourly"),
            Err(ScheduleIntervalError)
        );
        assert_eq!(ScheduleInterval::from_seconds(60), None);
    }
}
