use crate::cmd::{Cli, Command, StringFilter};
use crate::device_wrapper::DeviceWrapper;
use crate::display::{DevicePrinter, DisplayOpts};
use crate::traits::Execute;
use log::{info, warn};

const NO_DEVICES_HINT: &str = "no input device could be opened; reading /dev/input requires root access";

impl Execute for Cli {
    fn execute(self) -> anyhow::Result<()> {
        self.command.execute()
    }
}

impl Execute for Command {
    fn execute(self) -> anyhow::Result<()> {
        match self {
            cmd @ Command::List { .. } => {
                let mut stdout = std::io::stdout();
                let display_opts = DisplayOpts::try_from(cmd)?;

                let devices = DeviceWrapper::list_wrapped_devices();
                if devices.is_empty() {
                    warn!("Nothing to list: {NO_DEVICES_HINT}");
                }
                for device in devices {
                    DevicePrinter::new(&device, &display_opts).print(&mut stdout)?;
                }
            }
            Command::DeChatter {
                timeouts,
                name,
                path,
                physical_path,
                index,
            } => {
                let devices = DeviceWrapper::list_wrapped_devices();
                if devices.is_empty() {
                    anyhow::bail!("Nothing to de-chatter: {NO_DEVICES_HINT}");
                }

                let mut device_wrappers = filter_devices(devices, &name, &path, &physical_path);
                for (index, device) in device_wrappers.iter().enumerate() {
                    info!(
                        "A device with index={index} after applying filters: {} {} [{}]",
                        device.name(),
                        device.device.physical_path().unwrap_or_default(),
                        device.path.display()
                    );
                }
                if device_wrappers.is_empty() {
                    anyhow::bail!("No device found for given filters");
                }
                if index >= device_wrappers.len() {
                    anyhow::bail!(
                        "Device index {index} is out of range: only {} device(s) passed the filters",
                        device_wrappers.len()
                    );
                }
                let device_wrapper = device_wrappers.swap_remove(index);
                let mut filter = device_wrapper.build_key_filter(&timeouts)?;
                filter.block()?;
            }
        }

        Ok(())
    }
}

fn filter_devices(
    devices: Vec<DeviceWrapper>,
    name: &[StringFilter],
    path: &[StringFilter],
    physical_path: &[StringFilter],
) -> Vec<DeviceWrapper> {
    devices
        .into_iter()
        .filter(|device_wrapper| {
            path.iter()
                .all(|filter| filter.matches(device_wrapper.path.display().to_string()))
        })
        .filter(|device_wrapper| {
            name.iter()
                .all(|filter| filter.matches(device_wrapper.device.name().unwrap_or_default()))
        })
        .filter(|device_wrapper| {
            physical_path
                .iter()
                .all(|filter| filter.matches(device_wrapper.device.physical_path().unwrap_or_default()))
        })
        .collect::<Vec<_>>()
}
