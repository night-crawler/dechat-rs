use std::os::fd::AsFd as _;
use std::time::{Duration, Instant, SystemTime};

use evdev::uinput::VirtualDevice;
use evdev::{Device, EvdevEnum, EventSummary, InputEvent, SynchronizationCode};
use log::info;
use nix::errno::Errno;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};

use crate::cmd::KeyRangeTimeout;
use crate::debouncer::Debouncer;

const STATS_INTERVAL: Duration = Duration::from_secs(30);

struct ReportWriter {
    fake_keyboard: VirtualDevice,
    report: Vec<InputEvent>,
}

impl ReportWriter {
    fn emit_report(&mut self) -> std::io::Result<()> {
        if self.report.is_empty() {
            return Ok(());
        }
        self.fake_keyboard.emit(&self.report)?;
        self.report.clear();
        Ok(())
    }
}

pub struct KeyFilter {
    debouncer: Debouncer,
    orig_keyboard: Device,
    report_writer: ReportWriter,
    last_stats_printed: Instant,
}

impl KeyFilter {
    pub fn new(timeouts: &[KeyRangeTimeout], orig_keyboard: Device, fake_keyboard: VirtualDevice) -> Self {
        let max_keyboard_code = orig_keyboard
            .supported_keys()
            .iter()
            .flat_map(|attribute_set| attribute_set.iter())
            .map(EvdevEnum::to_index)
            .max()
            .unwrap_or_default();

        Self {
            debouncer: Debouncer::new(max_keyboard_code, timeouts),
            orig_keyboard,
            report_writer: ReportWriter {
                fake_keyboard,
                report: Vec::new(),
            },
            last_stats_printed: Instant::now(),
        }
    }

    pub fn block(&mut self) -> anyhow::Result<()> {
        self.wait_idle()?;

        self.orig_keyboard.grab()?;
        info!(
            "Grabbed the original keyboard: {}",
            self.orig_keyboard.physical_path().unwrap_or_default()
        );

        loop {
            match self.debouncer.next_release_deadline() {
                // No release is waiting, so it is safe to block until the keyboard sends something.
                None => self.process_event_batch()?,
                Some(deadline) => {
                    if self.events_arrive_before(deadline)? {
                        self.process_event_batch()?;
                    }
                    self.debouncer
                        .forward_releases_due_by(SystemTime::now(), &mut self.report_writer.report);
                    self.report_writer.emit_report()?;
                }
            }
            self.print_stats();
        }
    }

    fn wait_idle(&mut self) -> anyhow::Result<()> {
        // After the grab, the other readers get nothing more from the original keyboard.
        // A key that is held right now would stay pressed for them forever.
        // When we are started from a terminal, that key is Enter.
        let mut is_announced = false;
        loop {
            let held = self.orig_keyboard.get_key_state()?.iter().count();
            if held == 0 {
                return Ok(());
            }
            if !is_announced {
                info!("Waiting for {held} held key(s) to be released before grabbing the original keyboard");
                is_announced = true;
            }
            // The keyboard is not grabbed yet, so the other readers get these events from the
            // original keyboard.
            // Forwarding them would make every key arrive twice.
            self.orig_keyboard.fetch_events()?.for_each(drop);
        }
    }

    fn events_arrive_before(&self, deadline: SystemTime) -> anyhow::Result<bool> {
        let remaining = deadline.duration_since(SystemTime::now()).unwrap_or_default();
        // `poll` takes whole milliseconds, so round up. Rounded down, `poll` would return just before
        // the deadline with nothing due, and the loop would spin until the deadline came.
        let millis = remaining.as_nanos().div_ceil(1_000_000);
        let timeout = PollTimeout::try_from(millis).unwrap_or(PollTimeout::MAX);

        let mut fds = [PollFd::new(self.orig_keyboard.as_fd(), PollFlags::POLLIN)];
        match poll(&mut fds, timeout) {
            Ok(ready_count) => Ok(ready_count > 0),
            // A signal ended the wait early. The loop checks for due releases before it waits again.
            Err(Errno::EINTR) => Ok(false),
            Err(err) => Err(err.into()),
        }
    }

    fn process_event_batch(&mut self) -> anyhow::Result<()> {
        for event in self.orig_keyboard.fetch_events()? {
            match event.destructure() {
                EventSummary::Synchronization(_, SynchronizationCode::SYN_REPORT, _) => {
                    self.report_writer.emit_report()?;
                }
                EventSummary::Key(key_event, ..) => {
                    self.debouncer.on_key(key_event, &mut self.report_writer.report);
                }
                _ => self.report_writer.report.push(event),
            }
        }
        Ok(())
    }

    fn print_stats(&mut self) {
        if !self.debouncer.has_unreported_stats() || self.last_stats_printed.elapsed() < STATS_INTERVAL {
            return;
        }
        self.last_stats_printed = Instant::now();
        info!("Throttled: {}", self.debouncer.flush_report_stats());
    }
}
