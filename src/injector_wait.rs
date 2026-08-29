use std::time::{Duration, Instant};

pub fn find_pid_with_wait<T>(
    mut lookup: impl FnMut() -> Option<T>,
    wait_seconds: u64,
) -> Option<T> {
    let started_at = Instant::now();
    let wait_duration = Duration::from_secs(wait_seconds);
    loop {
        if let Some(value) = lookup() {
            return Some(value);
        }
        if wait_seconds == 0 || started_at.elapsed() >= wait_duration {
            return None;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

#[cfg(test)]
mod tests {
    use super::find_pid_with_wait;

    #[test]
    fn returns_value_on_immediate_lookup() {
        // Given a lookup that succeeds on its first call
        let mut lookups = 0;
        let lookup = || {
            lookups += 1;
            Some(42u32)
        };

        // When looking up with a nonzero wait
        let result = find_pid_with_wait(lookup, 30);

        // Then the value comes back after exactly one lookup, with no sleep
        assert_eq!(result, Some(42));
        assert_eq!(lookups, 1);
    }

    #[test]
    fn returns_none_after_single_lookup_when_wait_is_zero() {
        // Given a lookup that never succeeds and a zero-second wait
        let mut lookups = 0;
        let lookup = || {
            lookups += 1;
            None::<u32>
        };

        // When looking up with wait_seconds == 0
        let result = find_pid_with_wait(lookup, 0);

        // Then None is returned after exactly one lookup, with no sleep
        assert_eq!(result, None);
        assert_eq!(lookups, 1);
    }
}
