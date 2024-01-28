use std::time::{Duration, SystemTime, SystemTimeError};

#[derive(Debug, Clone)]
pub(super) enum KeyState {
    Down(SystemTime),
    Up(SystemTime),
}

impl Default for KeyState {
    fn default() -> Self {
        KeyState::Up(SystemTime::UNIX_EPOCH)
    }
}

impl KeyState {
    pub(super) fn time(&self) -> SystemTime {
        match self {
            KeyState::Down(ts) => *ts,
            KeyState::Up(ts) => *ts,
        }
    }
    pub(super) fn duration_since(&self, now: &SystemTime) -> Result<Duration, SystemTimeError> {
        now.duration_since(self.time())
    }
}
