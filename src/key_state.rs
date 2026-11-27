use std::time::{Duration, SystemTime};

/// The meaning of the value of an `EV_KEY` event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Release,
    Press,
    /// The kernel's autorepeat of a key that is being held.
    Repeat,
}

impl KeyAction {
    pub const fn from_value(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Release),
            1 => Some(Self::Press),
            2 => Some(Self::Repeat),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterResponse {
    Forward,
    ChatterDetected,
    DropPressRelease,
    /// Keep the release and forward it at `until`, unless a press cancels it first; see [`KeyState::Releasing`].
    HoldRelease {
        until: SystemTime,
    },
    /// Drop the event: it belongs to a press that was not forwarded.
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Up {
        released: SystemTime,
    },
    Down {
        pressed: SystemTime,
    },
    /// Pressed for the readers, but already released by the original keyboard.
    // A release right after a press may be a bounce: the user still holds the key.
    // Sending it at once would turn that hold into a short tap, so the release waits for the timeout.
    Releasing {
        pressed: SystemTime,
        released: SystemTime,
    },
}

impl Default for KeyState {
    fn default() -> Self {
        KeyState::Up {
            released: SystemTime::UNIX_EPOCH,
        }
    }
}

impl KeyState {
    pub fn on_event(&mut self, action: KeyAction, at: SystemTime, timeout: Duration) -> FilterResponse {
        match (*self, action) {
            (KeyState::Up { released }, KeyAction::Press) => {
                if elapsed(released, at) < timeout {
                    return FilterResponse::ChatterDetected;
                }
                *self = KeyState::Down { pressed: at };
                FilterResponse::Forward
            }
            // In `Up`, these events belong to a press that was dropped as chatter, so the readers must not get them.
            // In `Releasing`, the key is already released, and only a faulty keyboard sends them.
            (KeyState::Up { .. } | KeyState::Releasing { .. }, KeyAction::Release | KeyAction::Repeat) => {
                FilterResponse::Skip
            }
            (KeyState::Down { pressed }, KeyAction::Release) => {
                if elapsed(pressed, at) < timeout {
                    *self = KeyState::Releasing { pressed, released: at };
                    return FilterResponse::HoldRelease {
                        until: pressed + timeout,
                    };
                }
                *self = KeyState::Up { released: at };
                FilterResponse::Forward
            }
            // Chatter comes from the contacts of the key. A repeat comes from the autorepeat timer, so it cannot be
            // chatter. Throttling repeats would only slow the autorepeat down.
            (KeyState::Down { .. }, KeyAction::Press | KeyAction::Repeat) => FilterResponse::Forward,
            (KeyState::Releasing { pressed, .. }, KeyAction::Press) => {
                *self = KeyState::Down { pressed };
                FilterResponse::DropPressRelease
            }
        }
    }

    pub const fn on_held_release_forwarded(&mut self) {
        if let KeyState::Releasing { released, .. } = *self {
            // Use the time when the key was really released. The release was sent later,
            // and counting the timeout from then would drop a real second tap.
            *self = KeyState::Up { released };
        }
    }
}

fn elapsed(since: SystemTime, at: SystemTime) -> Duration {
    // The clock can be set back (NTP, the user), and then `since` is later than `at`.
    // Treating that as "no time passed" would drop every press of the key until the clock caught up,
    // which can take hours. So it counts as "long ago".
    at.duration_since(since).unwrap_or(Duration::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIMEOUT: Duration = Duration::from_millis(70);

    fn at(millis: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000) + Duration::from_millis(millis)
    }

    #[test]
    fn forwards_press_and_release_of_a_long_press() {
        let mut state = KeyState::default();
        assert_eq!(
            state.on_event(KeyAction::Press, at(0), TIMEOUT),
            FilterResponse::Forward
        );
        assert_eq!(state, KeyState::Down { pressed: at(0) });
        assert_eq!(
            state.on_event(KeyAction::Release, at(70), TIMEOUT),
            FilterResponse::Forward
        );
        assert_eq!(state, KeyState::Up { released: at(70) });
    }

    #[test]
    fn throttles_press_that_follows_release_too_soon() {
        let mut state = KeyState::Up { released: at(0) };
        assert_eq!(
            state.on_event(KeyAction::Press, at(69), TIMEOUT),
            FilterResponse::ChatterDetected
        );
        assert_eq!(
            state,
            KeyState::Up { released: at(0) },
            "a throttled press must not restart the timeout"
        );
        assert_eq!(
            state.on_event(KeyAction::Release, at(75), TIMEOUT),
            FilterResponse::Skip
        );
        assert_eq!(state, KeyState::Up { released: at(0) });
    }

    #[test]
    fn forwards_press_once_the_timeout_has_passed() {
        let mut state = KeyState::Up { released: at(0) };
        assert_eq!(
            state.on_event(KeyAction::Press, at(70), TIMEOUT),
            FilterResponse::Forward
        );
        assert_eq!(state, KeyState::Down { pressed: at(70) });
    }

    #[test]
    fn holds_back_release_of_a_short_press() {
        let mut state = KeyState::Down { pressed: at(0) };
        assert_eq!(
            state.on_event(KeyAction::Release, at(3), TIMEOUT),
            FilterResponse::HoldRelease { until: at(70) }
        );
        assert_eq!(
            state,
            KeyState::Releasing {
                pressed: at(0),
                released: at(3),
            }
        );
    }

    #[test]
    fn bounce_keeps_the_key_pressed() {
        let mut state = KeyState::Releasing {
            pressed: at(0),
            released: at(3),
        };
        assert_eq!(
            state.on_event(KeyAction::Press, at(6), TIMEOUT),
            FilterResponse::DropPressRelease
        );
        assert_eq!(state, KeyState::Down { pressed: at(0) });
        assert_eq!(
            state.on_event(KeyAction::Release, at(500), TIMEOUT),
            FilterResponse::Forward
        );
    }

    #[test]
    fn forwarded_held_release_keeps_the_time_of_the_original_release() {
        let mut state = KeyState::Releasing {
            pressed: at(0),
            released: at(3),
        };
        state.on_held_release_forwarded();
        assert_eq!(state, KeyState::Up { released: at(3) });
        assert_eq!(
            state.on_event(KeyAction::Press, at(72), TIMEOUT),
            FilterResponse::ChatterDetected
        );
        assert_eq!(
            state.on_event(KeyAction::Press, at(73), TIMEOUT),
            FilterResponse::Forward
        );
    }

    #[test]
    fn forwards_repeats_of_a_forwarded_press_only() {
        let mut state = KeyState::Down { pressed: at(0) };
        assert_eq!(
            state.on_event(KeyAction::Repeat, at(10), TIMEOUT),
            FilterResponse::Forward
        );
        assert_eq!(
            state,
            KeyState::Down { pressed: at(0) },
            "a repeat must not restart the press"
        );

        let mut state = KeyState::Up { released: at(0) };
        assert_eq!(
            state.on_event(KeyAction::Repeat, at(300), TIMEOUT),
            FilterResponse::Skip
        );
        assert_eq!(
            state,
            KeyState::Up { released: at(0) },
            "a repeat must not press the key"
        );
    }

    #[test]
    fn follows_the_clock_when_it_is_set_back() {
        let mut state = KeyState::Up { released: at(1000) };
        assert_eq!(
            state.on_event(KeyAction::Press, at(0), TIMEOUT),
            FilterResponse::Forward
        );
        assert_eq!(state, KeyState::Down { pressed: at(0) });

        let mut state = KeyState::Down { pressed: at(1000) };
        assert_eq!(
            state.on_event(KeyAction::Release, at(0), TIMEOUT),
            FilterResponse::Forward
        );
        assert_eq!(state, KeyState::Up { released: at(0) });
        assert_eq!(
            state.on_event(KeyAction::Press, at(10), TIMEOUT),
            FilterResponse::ChatterDetected
        );
    }

    #[test]
    fn knows_the_key_event_values() {
        assert_eq!(KeyAction::from_value(0), Some(KeyAction::Release));
        assert_eq!(KeyAction::from_value(1), Some(KeyAction::Press));
        assert_eq!(KeyAction::from_value(2), Some(KeyAction::Repeat));
        assert_eq!(KeyAction::from_value(3), None);
    }
}
