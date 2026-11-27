use std::time::{Duration, SystemTime};

use evdev::{EvdevEnum as _, InputEvent, KeyCode, KeyEvent};
use log::{debug, warn};

use crate::cmd::KeyRangeTimeout;
use crate::key_state::{FilterResponse, KeyAction, KeyState};

struct HeldRelease {
    event: KeyEvent,
    deadline: SystemTime,
}

pub struct Debouncer {
    key_timeouts: Vec<Option<Duration>>,
    tracker: Vec<KeyState>,
    held_releases: Vec<HeldRelease>,
    stats: Vec<usize>,
    has_unreported_stats: bool,
}

impl Debouncer {
    pub fn new(max_keyboard_code: usize, timeouts: &[KeyRangeTimeout]) -> Self {
        let max_requested_key_code = timeouts
            .iter()
            .map(|key_range_timeout| usize::from(*key_range_timeout.range.end()))
            .max()
            .unwrap_or_default();

        let required_size = max_keyboard_code.min(max_requested_key_code) + 1;
        let mut key_timeouts = vec![None; required_size];

        for key_range_timeout in timeouts {
            for key_code in key_range_timeout.range.clone().map(usize::from) {
                if key_code >= required_size {
                    warn!(
                        "Key code {} from provided range {:?} is out of keyboard's range: its highest key code is {}",
                        key_code, key_range_timeout, max_keyboard_code
                    );
                    break;
                }

                if let Some(timeout) = key_timeouts[key_code] {
                    warn!(
                        "Key code {:?} is already throttled with timeout {:?}, ignoring the new timeout {:?}",
                        key_code, timeout, key_range_timeout.timeout
                    );
                    continue;
                }

                key_timeouts[key_code] = Some(key_range_timeout.timeout);
            }
        }

        Self {
            tracker: vec![KeyState::default(); required_size],
            stats: vec![0; required_size],
            key_timeouts,
            held_releases: Vec::new(),
            has_unreported_stats: false,
        }
    }

    pub fn on_key(&mut self, event: KeyEvent, out: &mut Vec<InputEvent>) {
        let at = event.timestamp();
        // We can run late and get several old events at once.
        // A release may have been due between two of them, so it goes out now, before this event.
        // Otherwise a real second tap would look like a bounce, and two taps would become one long press.
        self.forward_releases_due_by(at, out);

        let key = event.code();
        let index = key.to_index();
        let action = KeyAction::from_value(event.value());
        let timeout = self.key_timeouts.get(index).copied().flatten();
        let filter_response = match (action, timeout) {
            (Some(action), Some(timeout)) => self.tracker[index].on_event(action, at, timeout),
            (None, _) | (_, None) => FilterResponse::Forward,
        };

        match filter_response {
            FilterResponse::Forward => {
                if action == Some(KeyAction::Press) {
                    // Keep the order of the keys. Example: the user lets go of Shift and then types "a".
                    // If the release of Shift were still held back, the readers would get "A".
                    self.forward_held_releases(|_| true, out);
                }
                out.push(event.into());
            }
            FilterResponse::ChatterDetected => self.count_throttled(key),
            FilterResponse::DropPressRelease => {
                self.held_releases.retain(|held| held.event.code() != key);
                self.count_throttled(key);
            }
            FilterResponse::HoldRelease { until } => self.held_releases.push(HeldRelease { event, deadline: until }),
            FilterResponse::Skip => {}
        }
    }

    pub fn next_release_deadline(&self) -> Option<SystemTime> {
        self.held_releases.iter().map(|held| held.deadline).min()
    }

    pub fn forward_releases_due_by(&mut self, time: SystemTime, out: &mut Vec<InputEvent>) {
        self.forward_held_releases(|held| held.deadline <= time, out);
    }

    pub const fn has_unreported_stats(&self) -> bool {
        self.has_unreported_stats
    }

    pub fn flush_report_stats(&mut self) -> String {
        self.has_unreported_stats = false;

        let mut parts = vec![];
        for (index, &count) in self.stats.iter().enumerate() {
            if count == 0 {
                continue;
            }
            let key_code = KeyCode::from_index(index);
            parts.push(format!("{key_code:?}:{index}x{count}"));
        }
        parts.join(", ")
    }

    fn forward_held_releases(&mut self, mut is_selected: impl FnMut(&HeldRelease) -> bool, out: &mut Vec<InputEvent>) {
        let tracker = &mut self.tracker;
        self.held_releases.retain(|held| {
            if !is_selected(held) {
                return true;
            }
            tracker[held.event.code().to_index()].on_held_release_forwarded();
            out.push(held.event.into());
            false
        });
    }

    fn count_throttled(&mut self, key: KeyCode) {
        debug!("Throttled a press of {key:?}:{}", key.code());
        let count = &mut self.stats[key.to_index()];
        *count = count.saturating_add(1);
        self.has_unreported_stats = true;
    }
}

#[cfg(test)]
mod tests {
    use evdev::{EventSummary, EventType};
    use nix::libc;

    use super::*;

    const TIMEOUT: Duration = Duration::from_millis(70);

    const A: KeyCode = KeyCode::KEY_A;
    const S: KeyCode = KeyCode::KEY_S;
    const SHIFT: KeyCode = KeyCode::KEY_LEFTSHIFT;
    const KEY_WITHOUT_TIMEOUT: KeyCode = KeyCode::KEY_F1;

    const RELEASE: i32 = 0;
    const PRESS: i32 = 1;
    const REPEAT: i32 = 2;

    fn at(millis: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000) + Duration::from_millis(millis)
    }

    fn key_event_at(key: KeyCode, value: i32, millis: u64) -> KeyEvent {
        let since_epoch = at(millis).duration_since(SystemTime::UNIX_EPOCH).unwrap();
        let raw = libc::input_event {
            time: libc::timeval {
                tv_sec: since_epoch.as_secs().try_into().unwrap(),
                tv_usec: since_epoch.subsec_micros().into(),
            },
            type_: EventType::KEY.0,
            code: key.code(),
            value,
        };
        match InputEvent::from(raw).destructure() {
            EventSummary::Key(event, ..) => event,
            _ => unreachable!("the event was built with EventType::KEY"),
        }
    }

    struct Harness {
        debouncer: Debouncer,
        out: Vec<InputEvent>,
    }

    impl Harness {
        fn new() -> Self {
            let timeouts = [KeyRangeTimeout {
                range: A.code()..=SHIFT.code(),
                timeout: TIMEOUT,
            }];
            Self {
                debouncer: Debouncer::new(255, &timeouts),
                out: Vec::new(),
            }
        }

        fn key(&mut self, key: KeyCode, value: i32, millis: u64) {
            self.debouncer.on_key(key_event_at(key, value, millis), &mut self.out);
        }

        fn forward_releases_due_by(&mut self, millis: u64) {
            self.debouncer.forward_releases_due_by(at(millis), &mut self.out);
        }

        fn forwarded(&self) -> Vec<(KeyCode, i32)> {
            self.out
                .iter()
                .map(|event| (KeyCode::new(event.code()), event.value()))
                .collect()
        }

        fn deadline_millis(&self) -> Option<u128> {
            let deadline = self.debouncer.next_release_deadline()?;
            Some(deadline.duration_since(at(0)).unwrap().as_millis())
        }
    }

    #[test]
    fn removes_chatter_on_release() {
        let mut harness = Harness::new();
        harness.key(A, PRESS, 0);
        harness.key(A, RELEASE, 100);
        harness.key(A, PRESS, 103);
        harness.key(A, RELEASE, 106);

        assert_eq!(harness.forwarded(), [(A, PRESS), (A, RELEASE)]);
        assert_eq!(harness.deadline_millis(), None);
        assert!(harness.debouncer.has_unreported_stats());
        assert_eq!(harness.debouncer.flush_report_stats(), "KEY_A:30x1");
        assert!(!harness.debouncer.has_unreported_stats());
    }

    #[test]
    fn keeps_a_key_held_through_chatter_on_press() {
        let mut harness = Harness::new();
        harness.key(A, PRESS, 0);
        harness.key(A, RELEASE, 3);
        assert_eq!(harness.deadline_millis(), Some(70));
        harness.key(A, PRESS, 6);
        assert_eq!(harness.deadline_millis(), None);
        harness.key(A, REPEAT, 256);
        assert_eq!(harness.forwarded(), [(A, PRESS), (A, REPEAT)]);

        harness.key(A, RELEASE, 500);
        assert_eq!(harness.forwarded().last(), Some(&(A, RELEASE)));
        assert_eq!(harness.debouncer.flush_report_stats(), "KEY_A:30x1");
    }

    #[test]
    fn releases_a_short_tap_when_the_timeout_has_passed() {
        let mut harness = Harness::new();
        harness.key(A, PRESS, 0);
        harness.key(A, RELEASE, 30);
        assert_eq!(harness.deadline_millis(), Some(70));

        harness.forward_releases_due_by(69);
        assert_eq!(harness.forwarded(), [(A, PRESS)]);

        harness.forward_releases_due_by(70);
        assert_eq!(harness.forwarded(), [(A, PRESS), (A, RELEASE)]);
        assert_eq!(harness.deadline_millis(), None);
        assert!(!harness.debouncer.has_unreported_stats());
    }

    #[test]
    fn releases_before_a_press_that_comes_after_the_deadline() {
        let mut harness = Harness::new();
        harness.key(A, PRESS, 0);
        harness.key(A, RELEASE, 30);
        harness.key(A, PRESS, 120);

        assert_eq!(harness.forwarded(), [(A, PRESS), (A, RELEASE), (A, PRESS)]);
        assert_eq!(harness.deadline_millis(), None);
    }

    #[test]
    fn releases_before_a_later_event_of_another_key() {
        let mut harness = Harness::new();
        harness.key(A, PRESS, 0);
        harness.key(A, RELEASE, 30);
        harness.key(S, RELEASE, 80);

        assert_eq!(harness.forwarded(), [(A, PRESS), (A, RELEASE)]);
        assert_eq!(harness.deadline_millis(), None);
    }

    #[test]
    fn releases_before_a_press_of_another_key() {
        let mut harness = Harness::new();
        harness.key(SHIFT, PRESS, 0);
        harness.key(A, PRESS, 20);
        harness.key(SHIFT, RELEASE, 40);
        harness.key(S, PRESS, 60);
        harness.key(A, RELEASE, 65);
        harness.key(KEY_WITHOUT_TIMEOUT, PRESS, 66);

        assert_eq!(
            harness.forwarded(),
            [
                (SHIFT, PRESS),
                (A, PRESS),
                (SHIFT, RELEASE),
                (S, PRESS),
                (A, RELEASE),
                (KEY_WITHOUT_TIMEOUT, PRESS),
            ]
        );
        assert_eq!(harness.deadline_millis(), None);
    }

    #[test]
    fn forwards_keys_without_a_timeout_untouched() {
        let mut harness = Harness::new();
        harness.key(KEY_WITHOUT_TIMEOUT, PRESS, 0);
        harness.key(KEY_WITHOUT_TIMEOUT, RELEASE, 1);
        harness.key(KEY_WITHOUT_TIMEOUT, PRESS, 2);

        assert_eq!(
            harness.forwarded(),
            [
                (KEY_WITHOUT_TIMEOUT, PRESS),
                (KEY_WITHOUT_TIMEOUT, RELEASE),
                (KEY_WITHOUT_TIMEOUT, PRESS),
            ]
        );
        assert!(!harness.debouncer.has_unreported_stats());
    }

    #[test]
    fn limits_timeouts_to_the_key_codes_of_the_keyboard() {
        let timeouts = [
            KeyRangeTimeout {
                range: 10..=20,
                timeout: Duration::from_millis(5),
            },
            KeyRangeTimeout {
                range: 0..=1000,
                timeout: TIMEOUT,
            },
        ];
        let debouncer = Debouncer::new(50, &timeouts);

        assert_eq!(debouncer.key_timeouts.len(), 51);
        assert_eq!(debouncer.key_timeouts[9], Some(TIMEOUT));
        assert_eq!(debouncer.key_timeouts[10], Some(Duration::from_millis(5)));
        assert_eq!(debouncer.key_timeouts[20], Some(Duration::from_millis(5)));
        assert_eq!(debouncer.key_timeouts[50], Some(TIMEOUT));
    }
}
