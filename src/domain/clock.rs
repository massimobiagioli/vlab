//! Injectable sleep for deterministic retry tests (AD-8).

use std::cell::RefCell;
use std::time::Duration;

/// The only time-related operation the retry policy needs.
pub trait Clock {
    fn sleep(&self, duration: Duration);
}

#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// Records requested sleeps instead of waiting.
#[derive(Debug, Default)]
pub struct TestClock {
    sleeps: RefCell<Vec<Duration>>,
}

impl TestClock {
    pub fn sleeps(&self) -> Vec<Duration> {
        self.sleeps.borrow().clone()
    }
}

impl Clock for TestClock {
    fn sleep(&self, duration: Duration) {
        self.sleeps.borrow_mut().push(duration);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_test_clock_records_without_sleeping() {
        let clock = TestClock::default();
        clock.sleep(Duration::from_millis(500));
        clock.sleep(Duration::from_secs(1));
        assert_eq!(
            clock.sleeps(),
            [Duration::from_millis(500), Duration::from_secs(1)]
        );
    }
}
