use std::hint::unreachable_unchecked;
use std::time::{Duration, Instant};

use evdev::uinput::VirtualDevice;
use evdev::{Device, EvdevEnum, EventSummary, InputEvent, KeyCode};
use log::{debug, error, info, warn};

use crate::cmd::KeyRangeTimeout;
use crate::key_state::KeyState;

pub(super) struct KeyFilter {
    key_timeouts: Vec<Option<Duration>>,
    tracker: Vec<KeyState>,
    orig_keyboard: Device,
    fake_keyboard: VirtualDevice,
    stats: Vec<usize>,
    last_stats_printed: Instant,
}

impl KeyFilter {
    pub(super) fn new(
        timeouts: Vec<KeyRangeTimeout>,
        orig_keyboard: Device,
        fake_keyboard: VirtualDevice,
    ) -> Self {
        let max_keyboard_code = orig_keyboard
            .supported_keys()
            .iter()
            .flat_map(|attribute_set| attribute_set.iter())
            .map(|key_code| key_code.to_index())
            .max()
            .unwrap_or_default();

        let max_requested_key_code = timeouts
            .iter()
            .map(|key_range_timeout| (*key_range_timeout.range.end()) as usize)
            .max()
            .unwrap_or_default();

        let required_size = max_keyboard_code.min(max_requested_key_code) + 1;
        let mut key_timeouts = vec![None; required_size];

        for key_range_timeout in timeouts {
            for key_code in key_range_timeout.range.clone().map(usize::from) {
                if key_code >= required_size {
                    warn!("Key code from provided range {:?} is out of keyboard's range: keyboard has {} key codes", key_range_timeout, max_keyboard_code);
                    break;
                }

                if let Some(timeout) = key_timeouts[key_code] {
                    warn!("Key code {:?} is already throttled with timeout {:?}, ignoring the new timeout {:?}", key_code, timeout, key_range_timeout.timeout);
                    continue;
                }

                key_timeouts[key_code] = Some(key_range_timeout.timeout);
            }
        }

        Self {
            tracker: vec![KeyState::default(); required_size],
            stats: vec![0; required_size],
            key_timeouts,
            orig_keyboard,
            fake_keyboard,
            last_stats_printed: Instant::now(),
        }
    }

    pub(super) fn block(&mut self) -> anyhow::Result<()> {
        self.orig_keyboard.grab()?;
        info!("Grabbed the original keyboard");

        loop {
            self.process_event_batch()?;
        }
    }

    fn process_event_batch(&mut self) -> anyhow::Result<()> {
        let mut filtered = false;
        for orig_event in self.orig_keyboard.fetch_events()? {
            if Self::should_filter(orig_event, &self.key_timeouts, &mut self.tracker) {
                filtered = true;
                let index = orig_event.code() as usize;
                self.stats[index] = self.stats[index].saturating_add(1);
                continue;
            }
            self.fake_keyboard.emit(&[orig_event])?;
        }

        if filtered {
            self.print_stats();
        }
        Ok(())
    }
    fn should_filter(
        orig_event: InputEvent,
        key_timeouts: &[Option<Duration>],
        tracker: &mut [KeyState],
    ) -> bool {
        let (key_code, key_state) = match orig_event.destructure() {
            EventSummary::Key(_, key_code, key_state) => (key_code, key_state),
            _ => return false,
        };
        let Some(&Some(max_duration)) = key_timeouts.get(key_code.to_index()) else {
            debug!("Key code {key_code:?} cannot be throttled");
            return false;
        };

        let now = orig_event.timestamp();
        let state = &mut tracker[key_code.to_index()];

        let since_previous = match state.duration_since(&now) {
            Ok(value) => value,
            Err(err) => {
                error!("Clock drift detected; skipping filtering: {err}");
                return false;
            }
        };

        let is_key_down = key_state >= 1;

        match state {
            KeyState::Down(ts) if is_key_down => {
                // It was pressed and remains pressed; probably we would not like to throttle that
                // Or we'd like to configure what key codes we need to throttle here
                if since_previous < max_duration {
                    debug!(
                        "Throttled repeated down-down {key_code:?}:{}; elapsed: {}",
                        key_code.code(),
                        since_previous.as_millis()
                    );
                    return true;
                }
                *ts = now;
                false
            }
            KeyState::Down(_) if !is_key_down => {
                // It is released now; we change the state to Up;
                *state = KeyState::Up(now);
                false
            }
            KeyState::Up(_) if is_key_down => {
                // It was released some time ago, and now it's pressed again
                // Not to confuse the next State::Up statement we change the state always
                // *state = KeyState::Down(*prev);
                if since_previous < max_duration {
                    debug!(
                        "Throttled repeated up-down {key_code:?}:{}; elapsed: {}",
                        key_code.code(),
                        since_previous.as_millis()
                    );
                    return true;
                }
                *state = KeyState::Down(now);
                false
            }
            KeyState::Up(_) if !is_key_down => {
                // It was released twice? Did we loose an event? I'd say we do nothing
                debug!(
                    "Unconditionally throttled repeated up-up {key_code:?}:{}; elapsed: {} (elapsed is ignored)",
                        key_code.code(),
                        since_previous.as_millis()
                    );
                true
            }
            _ => unsafe { unreachable_unchecked() },
        }
    }

    fn print_stats(&mut self) {
        if self.last_stats_printed.elapsed() < Duration::from_secs(30) {
            return;
        }
        self.last_stats_printed = Instant::now();
        let mut parts = vec![];

        for (index, &count) in self.stats.iter().enumerate() {
            if count == 0 {
                continue;
            }
            let key_code = KeyCode::from_index(index);
            parts.push(format!("{key_code:?}:{index}x{count}"))
        }

        if parts.is_empty() {
            return;
        }

        info!("Throttled: {}", parts.join(", "));
    }
}
