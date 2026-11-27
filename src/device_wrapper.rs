use crate::cmd::KeyRangeTimeout;
use crate::key_filter::KeyFilter;
use crate::traits::DeviceExt as _;
use evdev::uinput::VirtualDevice;
use evdev::{Device, UinputAbsSetup};
use log::info;
use std::cmp::Ordering;
use std::path::PathBuf;

// `VirtualDeviceBuilder::build` panics on a longer name, so with a long keyboard name we would
// crash at start. One byte is for the NUL terminator, and `build` wants one more free:
// https://docs.rs/evdev/0.13.2/src/evdev/uinput.rs.html#235
const MAX_FAKE_NAME_LEN: usize = nix::libc::UINPUT_MAX_NAME_SIZE - 2;

pub struct DeviceWrapper {
    pub path: PathBuf,
    pub device: Device,
}

impl DeviceWrapper {
    pub fn name(&self) -> &str {
        self.device.name().unwrap_or("Unnamed device")
    }

    fn ordering_key(&self) -> (u16, u16, u16, u16, &str, &PathBuf) {
        let id = self.device.input_id();
        (
            id.bus_type().0,
            id.vendor(),
            id.product(),
            id.version(),
            self.name(),
            &self.path,
        )
    }

    fn create_fake_keyboard(&self) -> std::io::Result<VirtualDevice> {
        let device = &self.device;
        let name = fake_keyboard_name(self.name());

        let mut builder = VirtualDevice::builder()?
            .name(&name)
            .input_id(device.input_id())
            .with_properties(device.properties())?;

        if let Some(keys) = device.supported_keys() {
            builder = builder.with_keys(keys)?;
        }
        if let Some(axes) = device.supported_relative_axes() {
            builder = builder.with_relative_axes(axes)?;
        }
        if device.supported_absolute_axes().is_some() {
            for (axis, info) in device.get_absinfo()? {
                builder = builder.with_absolute_axis(&UinputAbsSetup::new(axis, info))?;
            }
        }
        if let Some(switches) = device.supported_switches() {
            builder = builder.with_switches(switches)?;
        }
        if let Some(misc) = device.misc_properties() {
            builder = builder.with_msc(misc)?;
        }

        builder.build()
    }

    pub fn build_key_filter(self, timeouts: &[KeyRangeTimeout]) -> anyhow::Result<KeyFilter> {
        info!("Picked the original keyboard: {}; path: {:?}", self.name(), self.path);

        let mut fake_keyboard = self.create_fake_keyboard()?;

        let paths = fake_keyboard.paths()?;
        info!("Created a fake keyboard; it is available as {:?}", paths);

        Ok(KeyFilter::new(timeouts, self.device, fake_keyboard))
    }

    pub fn list_wrapped_devices() -> Vec<DeviceWrapper> {
        let mut devices = evdev::enumerate().map(DeviceWrapper::from).collect::<Vec<_>>();
        devices.sort_unstable();
        devices
    }
}

fn fake_keyboard_name(original: &str) -> String {
    let mut name = format!("De-chattered Keyboard: {original}");
    let mut end = name.len().min(MAX_FAKE_NAME_LEN);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    name.truncate(end);
    name
}

impl From<(PathBuf, Device)> for DeviceWrapper {
    fn from((path, device): (PathBuf, Device)) -> Self {
        Self { path, device }
    }
}

impl PartialEq<Self> for DeviceWrapper {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Eq for DeviceWrapper {}

impl PartialOrd<Self> for DeviceWrapper {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DeviceWrapper {
    fn cmp(&self, other: &Self) -> Ordering {
        self.ordering_key().cmp(&other.ordering_key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_a_short_name() {
        assert_eq!(
            fake_keyboard_name("Asus Keyboard"),
            "De-chattered Keyboard: Asus Keyboard"
        );
    }

    #[test]
    fn cuts_a_long_name_at_a_char_boundary() {
        let prefix_len = fake_keyboard_name("").len();
        let one_byte_below_the_limit = "x".repeat(MAX_FAKE_NAME_LEN - prefix_len - 1);
        let two_byte_char_across_the_limit = format!("{one_byte_below_the_limit}®");

        assert_eq!(
            fake_keyboard_name(&two_byte_char_across_the_limit),
            fake_keyboard_name(&one_byte_below_the_limit)
        );
        assert_eq!(fake_keyboard_name(&"x".repeat(200)).len(), MAX_FAKE_NAME_LEN);
    }
}
