use crate::cmd::KeyRangeTimeout;
use crate::key_filter::KeyFilter;
use crate::traits::DeviceExt as _;
use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, Device, KeyCode};
use log::info;
use std::cmp::Ordering;
use std::path::PathBuf;

pub struct DeviceWrapper {
    pub path: PathBuf,
    pub device: Device,
}

impl DeviceWrapper {
    pub fn name(&self) -> &str {
        self.device.name().unwrap_or("Unnamed device")
    }

    fn get_ordering_key(&self) -> (u16, u16, u16, u16, &str, &PathBuf) {
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
        let mut keys = AttributeSet::<KeyCode>::new();
        for supported_key in self
            .device
            .supported_keys()
            .iter()
            .flat_map(|attribute_set| attribute_set.iter())
        {
            keys.insert(supported_key);
        }

        VirtualDevice::builder()?
            .name(&format!("De-chattered Keyboard: {}", self.name()))
            .with_keys(&keys)?
            .build()
    }

    pub fn build_key_filter(self, timeouts: Vec<KeyRangeTimeout>, skip_first: bool) -> anyhow::Result<KeyFilter> {
        info!("Picked the original keyboard: {}; path: {:?}", self.name(), self.path);

        let mut fake_keyboard = self.create_fake_keyboard()?;

        let paths = fake_keyboard.paths()?;
        info!("Created a fake keyboard; it is available as {:?}", paths);

        Ok(KeyFilter::new(timeouts, self.device, fake_keyboard, skip_first))
    }

    pub fn list_wrapped_divices() -> Vec<DeviceWrapper> {
        let mut devices = evdev::enumerate().map(DeviceWrapper::from).collect::<Vec<_>>();
        devices.sort_unstable();
        devices
    }
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
        self.get_ordering_key().cmp(&other.get_ordering_key())
    }
}
