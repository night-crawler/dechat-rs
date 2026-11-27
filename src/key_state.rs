use std::time::{Duration, SystemTime, SystemTimeError};

#[derive(Debug, Clone)]
pub enum KeyState {
    Down(SystemTime),
    Up(SystemTime),
}

impl Default for KeyState {
    fn default() -> Self {
        KeyState::Up(SystemTime::UNIX_EPOCH)
    }
}

impl KeyState {
    pub const fn time(&self) -> SystemTime {
        match self {
            KeyState::Down(ts) | KeyState::Up(ts) => *ts,
        }
    }
    pub fn duration_since(&self, now: &SystemTime) -> Result<Duration, SystemTimeError> {
        now.duration_since(self.time())
    }
}
