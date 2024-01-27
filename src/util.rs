use std::cmp::Ordering;
use std::path::PathBuf;

use evdev::Device;

pub(super) fn get_devices() -> Vec<DeviceWrapper> {
    let mut devices = evdev::enumerate()
        .map(DeviceWrapper::from)
        .collect::<Vec<_>>();
    devices.sort_unstable();
    devices
}

pub(super) struct DeviceWrapper {
    pub(super) path: PathBuf,
    pub(super) device: Device,
}

impl DeviceWrapper {}

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
        (get_device_ordering_key(&self.device), &self.path)
            .cmp(&(get_device_ordering_key(&other.device), &self.path))
    }
}

fn get_device_ordering_key(device: &Device) -> (u16, u16, u16, u16, &str) {
    let id = device.input_id();
    let name = device.name().unwrap_or("Unnamed device");
    (
        id.bus_type().0,
        id.vendor(),
        id.product(),
        id.version(),
        name,
    )
}
