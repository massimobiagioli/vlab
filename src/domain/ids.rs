//! Entity identifiers (ADR-0004, AD-5).
//!
//! Every identifier is exactly eight lowercase hexadecimal characters derived
//! from a UUIDv4. Parsing is strict: no trimming, case folding, or prefix
//! resolution. Generation is random, so repositories use [`unique_short_id`]
//! to retry a bounded number of times when the primary key already exists.

use std::fmt;

use uuid::Uuid;

/// Maximum number of generation attempts before a collision is fatal.
pub const MAX_ID_ATTEMPTS: usize = 5;

/// True for exactly 8 lowercase hexadecimal characters.
pub fn is_short_id(value: &str) -> bool {
    value.len() == 8
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A fresh random identifier in the canonical display format.
pub fn new_short_id() -> String {
    let hex = Uuid::new_v4().simple().to_string();
    hex[..8].to_owned()
}

/// Why a unique identifier could not be produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniqueIdError<E> {
    /// Every attempt collided.
    Exhausted,
    /// The existence check failed.
    Lookup(E),
}

/// Generates an identifier that `is_taken` reports as unused, retrying at most
/// [`MAX_ID_ATTEMPTS`] times. The closure performs the I/O; this function stays
/// pure and unit-testable.
pub fn unique_short_id<E>(
    mut is_taken: impl FnMut(&str) -> Result<bool, E>,
) -> Result<String, UniqueIdError<E>> {
    for _ in 0..MAX_ID_ATTEMPTS {
        let id = new_short_id();
        if !is_taken(&id).map_err(UniqueIdError::Lookup)? {
            return Ok(id);
        }
    }
    Err(UniqueIdError::Exhausted)
}

macro_rules! short_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(String);

        impl $name {
            /// Accepts only the ADR-0004 format; no trimming or case folding.
            pub fn parse(value: &str) -> Option<Self> {
                is_short_id(value).then(|| Self(value.to_owned()))
            }

            pub fn generate() -> Self {
                Self(new_short_id())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

short_id!(LabId);
short_id!(AgentId);
short_id!(ProjectId);
short_id!(RequestId);
short_id!(StoryId);
short_id!(MemoryId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_short_lowercase_hex_is_accepted() {
        assert_eq!(
            LabId::parse("a3f91c07").map(|id| id.to_string()),
            Some("a3f91c07".into())
        );
        for bad in [
            "",
            "a3f91c0",
            "a3f91c070",
            "A3F91C07",
            " a3f91c07",
            "a3f91c0g",
            "../../x",
        ] {
            assert!(LabId::parse(bad).is_none(), "{bad:?}");
            assert!(AgentId::parse(bad).is_none(), "{bad:?}");
            assert!(ProjectId::parse(bad).is_none(), "{bad:?}");
            assert!(RequestId::parse(bad).is_none(), "{bad:?}");
            assert!(StoryId::parse(bad).is_none(), "{bad:?}");
            assert!(MemoryId::parse(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn every_id_type_round_trips() {
        let lab = LabId::generate();
        let agent = AgentId::generate();
        let project = ProjectId::generate();
        let request = RequestId::generate();
        let story = StoryId::generate();
        for id in [
            lab.to_string(),
            agent.to_string(),
            project.to_string(),
            request.to_string(),
            story.to_string(),
        ] {
            assert!(is_short_id(&id), "{id}");
        }
        assert_eq!(LabId::parse(&lab.to_string()).as_ref(), Some(&lab));
        assert_eq!(AgentId::parse(&agent.to_string()).as_ref(), Some(&agent));
        assert_eq!(
            ProjectId::parse(&project.to_string()).as_ref(),
            Some(&project)
        );
        assert_eq!(
            RequestId::parse(&request.to_string()).as_ref(),
            Some(&request)
        );
        assert_eq!(StoryId::parse(&story.to_string()).as_ref(), Some(&story));
        let memory = MemoryId::generate();
        assert_eq!(MemoryId::parse(&memory.to_string()).as_ref(), Some(&memory));
    }

    #[test]
    fn generated_identifiers_are_distinct_and_short() {
        let first = new_short_id();
        let second = new_short_id();
        assert_ne!(first, second);
        assert_eq!(first.len(), 8);
        assert!(is_short_id(&first));
    }

    #[test]
    fn collision_retry_is_bounded_and_returns_the_first_free_identifier() {
        let mut calls = 0;
        let id = unique_short_id::<()>(|_| {
            calls += 1;
            Ok(calls <= 2)
        })
        .unwrap();
        assert_eq!(calls, 3);
        assert!(is_short_id(&id));

        let mut calls = 0;
        let exhausted = unique_short_id::<()>(|_| {
            calls += 1;
            Ok(true)
        });
        assert_eq!(exhausted, Err(UniqueIdError::Exhausted));
        assert_eq!(calls, MAX_ID_ATTEMPTS);
    }

    #[test]
    fn lookup_failures_are_returned_without_exhausting_attempts() {
        let error = unique_short_id(|_| Err::<bool, _>("offline")).unwrap_err();
        assert_eq!(error, UniqueIdError::Lookup("offline"));
    }
}
