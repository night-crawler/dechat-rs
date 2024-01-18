use std::hint::unreachable_unchecked;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, SystemTimeError};

use anyhow::Context;
use env_logger::Env;
use evdev::uinput::VirtualDevice;
use evdev::{uinput::VirtualDeviceBuilder, AttributeSet, Device, EvdevEnum, EventSummary, KeyCode};
use log::{error, info, warn};

pub fn pick_device() -> anyhow::Result<(PathBuf, Device)> {
    use std::io::prelude::*;

    let mut args = std::env::args_os();
    args.next();
    if let Some(dev_file) = args.next() {
        Ok((dev_file.clone().into(), Device::open(&dev_file)?))
    } else {
        let mut devices = evdev::enumerate().collect::<Vec<_>>();
        devices.reverse();
        for (i, (dev_file, device)) in devices.iter().enumerate() {
            println!(
                "{i}: {} ({})",
                dev_file.display(),
                device.name().unwrap_or("Unnamed device")
            );
        }
        print!("Select the device [0-{}]: ", devices.len());
        let _ = std::io::stdout().flush();
        let mut chosen = String::new();
        std::io::stdin().read_line(&mut chosen).unwrap();
        let n = chosen.trim().parse::<usize>().unwrap();

        Ok(devices.into_iter().nth(n).context("Incorrect index")?)
    }
}

#[derive(Debug, Clone)]
enum State {
    Down(SystemTime),
    Up(SystemTime),
}

impl Default for State {
    fn default() -> Self {
        State::Up(SystemTime::UNIX_EPOCH)
    }
}

impl State {
    fn time(&self) -> SystemTime {
        match self {
            State::Down(ts) => *ts,
            State::Up(ts) => *ts,
        }
    }
    fn duration_since(&self, now: &SystemTime) -> Result<Duration, SystemTimeError> {
        now.duration_since(self.time())
    }
}

fn main() -> anyhow::Result<()> {
    let env = Env::default()
        .filter_or("MY_LOG_LEVEL", "trace")
        .write_style_or("MY_LOG_STYLE", "always");

    env_logger::init_from_env(env);

    let max_duration = Duration::from_millis(40);

    let (dev_file, mut orig_keyboard) = pick_device()?;
    info!(
        "Picked the original keyboard: {}; path: {:?}",
        orig_keyboard.name().unwrap_or("Unnamed device"),
        dev_file
    );

    let mut keys = AttributeSet::<KeyCode>::new();
    for supported_key in orig_keyboard
        .supported_keys()
        .iter()
        .flat_map(|attribute_set| attribute_set.iter())
    {
        keys.insert(supported_key);
    }

    let mut fake_keyboard = VirtualDeviceBuilder::new()?
        .name("De-chattered Fake Keyboard")
        .with_keys(&keys)?
        .build()
        .unwrap();

    let paths = fake_keyboard.paths()?;
    info!("Created a fake keyboard; it is available as {:?}", paths);

    orig_keyboard.grab()?;
    info!("Grabbed the original keyboard");

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
                            error!("Clock drift: {err}");
                            fake_keyboard.emit(&[orig_event])?;
                            continue;
                        }
                    };

                    let is_key_down = key_state >= 1;

                    match state {
                        State::Down(ts) if is_key_down => {
                            // It was pressed and remains pressed; probably we would not like to throttle that
                            // Or we'd like to configure what key codes we need to throttle here
                            if since_previous < max_duration {
                                warn!(
                                    "Throttled repeated down-down {key_code:?}:{}; elapsed: {}",
                                    key_code.code(),
                                    since_previous.as_millis()
                                );
                                continue;
                            }
                            fake_keyboard.emit(&[orig_event])?;
                            *ts = now;
                        }
                        State::Down(_) if !is_key_down => {
                            // It is released now; we change the state to released
                            *state = State::Up(now);
                            fake_keyboard.emit(&[orig_event])?;
                        }
                        State::Up(_) if is_key_down => {
                            // It was released some time ago and now it's pressed again
                            // Not to confuse the next State::Release statement we change the state always
                            *state = State::Down(now);
                            if since_previous < max_duration {
                                warn!(
                                    "Throttled repeated down-up {key_code:?}:{}; elapsed: {}",
                                    key_code.code(),
                                    since_previous.as_millis()
                                );
                                continue;
                            }

                            fake_keyboard.emit(&[orig_event])?;
                        }
                        State::Up(_) if !is_key_down => {
                            // It was released twice? Did we loose an event? I'd say we do nothing
                            warn!(
                                "Unconditionally throttled repeated up-up {key_code:?}:{}; elapsed: {} (elapsed is ignored)",
                                key_code.code(),
                                since_previous.as_millis()
                            );
                            continue;
                        }
                        _ => unsafe { unreachable_unchecked() },
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

trait DeviceExt {
    fn paths(&mut self) -> anyhow::Result<Vec<PathBuf>>;
}

impl DeviceExt for VirtualDevice {
    fn paths(&mut self) -> anyhow::Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        for path in self.enumerate_dev_nodes_blocking()? {
            paths.push(path?);
        }
        Ok(paths)
    }
}
