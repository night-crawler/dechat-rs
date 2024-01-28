use crate::cmd::{Cli, Command};
use crate::display::{DevicePrinter, DisplayOpts};
use crate::traits::Execute;
use crate::util::get_devices;
use anyhow::Context;

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

                for device in get_devices() {
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
                let index = index.unwrap_or_default();
                let device_wrapper = get_devices()
                    .into_iter()
                    .filter(|device_wrapper| {
                        path.iter()
                            .all(|filter| filter.matches(device_wrapper.path.display().to_string()))
                    })
                    .filter(|device_wrapper| {
                        name.iter().all(|filter| {
                            filter.matches(device_wrapper.device.name().unwrap_or_default())
                        })
                    })
                    .filter(|device_wrapper| {
                        physical_path.iter().all(|filter| {
                            filter
                                .matches(device_wrapper.device.physical_path().unwrap_or_default())
                        })
                    })
                    .nth(index)
                    .with_context(|| "No device found for given filters")?;

                let mut filter = device_wrapper.de_chatter(timeouts)?;
                filter.block()?;
            }
        }

        Ok(())
    }
}
