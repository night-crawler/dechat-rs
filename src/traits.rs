use evdev::uinput::VirtualDevice;
use std::path::PathBuf;

pub(super) trait Execute {
    fn execute(self) -> anyhow::Result<()>;
}

pub(super) trait DeviceExt {
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
