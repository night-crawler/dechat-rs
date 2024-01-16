use std::hint::unreachable_unchecked;
use std::time::{Duration, SystemTime, SystemTimeError};

use anyhow::Context;
use evdev::{AttributeSet, EvdevEnum, EventSummary, KeyCode, uinput::VirtualDeviceBuilder};

pub fn pick_device() -> anyhow::Result<evdev::Device> {
    use std::io::prelude::*;

    let mut args = std::env::args_os();
    args.next();
    if let Some(dev_file) = args.next() {
        Ok(evdev::Device::open(dev_file)?)
    } else {
        let mut devices = evdev::enumerate().map(|t| t.1).collect::<Vec<_>>();
        devices.reverse();
        for (i, d) in devices.iter().enumerate() {
            println!("{}: {}", i, d.name().unwrap_or("Unnamed device"));
        }
        print!("Select the device [0-{}]: ", devices.len());
        let _ = std::io::stdout().flush();
        let mut chosen = String::new();
        std::io::stdin().read_line(&mut chosen).unwrap();
        let n = chosen.trim().parse::<usize>().unwrap();
        Ok(devices.into_iter().nth(n).context("No n")?)
    }
}

#[derive(Debug, Clone)]
enum State {
    Pressed(SystemTime),
    Released(SystemTime, Duration),
}

impl Default for State {
    fn default() -> Self {
        State::Released(SystemTime::UNIX_EPOCH, Duration::from_secs(0))
    }
}

impl State {
    fn time(&self) -> SystemTime {
        match self {
            State::Pressed(ts) => *ts,
            State::Released(ts, _) => *ts,
        }
    }
    fn duration_since(&self, now: &SystemTime) -> Result<Duration, SystemTimeError> {
        now.duration_since(self.time())
    }
}

fn main() -> anyhow::Result<()> {
    let mut orig_keyboard = pick_device()?;

    let mut keys = AttributeSet::<KeyCode>::new();
    for supported_key in orig_keyboard
        .supported_keys()
        .iter()
        .flat_map(|attribute_set| attribute_set.iter())
    {
        keys.insert(supported_key);
    }

    let mut fake_keyboard = VirtualDeviceBuilder::new()?
        .name("Fake Keyboard")
        .with_keys(&keys)?
        .build()
        .unwrap();

    for path in fake_keyboard.enumerate_dev_nodes_blocking()? {
        let path = path?;
        println!("Available as {}", path.display());
    }

    orig_keyboard.grab()?;

    let mut tracker = vec![State::default(); 0x2ff + 1];

    loop {
        for orig_event in orig_keyboard.fetch_events()? {
            match orig_event.destructure() {
                EventSummary::Key(_, key_code, key_state) => {
                    let now = orig_event.timestamp();
                    let state = &mut tracker[key_code.to_index()];

                    let since_previous = match state.duration_since(&now) {
                        Ok(value) => value,
                        Err(err) => {
                            println!("Clock drift: {err}");
                            fake_keyboard.emit(&[orig_event])?;
                            continue;
                        }
                    };

                    let is_key_down = key_state >= 1;

                    match state {
                        State::Pressed(ts) if is_key_down => {
                            // It was pressed and remains pressed; probably we would not like to throttle that
                            // Or we'd like to configure what key codes we need to throttle here
                            if since_previous < Duration::from_millis(40) {
                                println!("Throttle press {orig_event:?}");
                                continue;
                            }
                            *ts = now;
                        }
                        State::Pressed(_) if !is_key_down => {
                            // It is released now; we change the state to released
                            *state = State::Released(now, since_previous);
                            fake_keyboard.emit(&[orig_event])?;
                        }
                        State::Released(_, _) if is_key_down => {
                            // It was released some time ago and now it's pressed again

                            // Not to confuse the next State::Release statement we change the state always
                            *state = State::Pressed(now);
                            if since_previous < Duration::from_millis(40) {
                                println!("Throttle release {orig_event:?}");
                                continue;
                            }

                            fake_keyboard.emit(&[orig_event])?;
                        }
                        State::Released(_, _) if !is_key_down => {
                            // It was released twice? Did we loose an event? I'd say we do nothing
                            println!("Double release; event {orig_event:?}; [{key_state}]");
                            continue;
                        }
                        _ => unsafe { unreachable_unchecked() }
                    }
                }

                // we care only about pressed keys
                _ => {
                    fake_keyboard.emit(&[orig_event])?;
                }
            }
        }
    }
}
