use crate::cmd::{Cli, Command, StringFilter};
use crate::device_wrapper::DeviceWrapper;
use crate::display::{DevicePrinter, DisplayOpts};
use crate::traits::Execute;
use log::info;

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

                for device in DeviceWrapper::list_wrapped_divices() {
                    DevicePrinter::new(&device, &display_opts).print(&mut stdout)?;
                }
            }
            Command::DeChatter {
                timeouts,
                name,
                path,
                physical_path,
                index,
                skip_first,
            } => {
                let mut device_wrappers = get_filtered_devices(&name, &path, &physical_path);
                for (index, device) in device_wrappers.iter().enumerate() {
                    info!(
                        "A device with index={index} after applying filters: {} {} [{}]",
                        device.name(),
                        device.device.physical_path().unwrap_or_default(),
                        device.path.display()
                    );
                }
                if index >= device_wrappers.len() {
                    anyhow::bail!("No device found for given filters");
                }
                let device_wrapper = device_wrappers.swap_remove(index);
                let mut filter = device_wrapper.build_key_filter(timeouts, skip_first)?;
                filter.block()?;
            }
        }

        Ok(())
    }
}

fn get_filtered_devices(
    name: &[StringFilter],
    path: &[StringFilter],
    physical_path: &[StringFilter],
) -> Vec<DeviceWrapper> {
    DeviceWrapper::list_wrapped_divices()
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
