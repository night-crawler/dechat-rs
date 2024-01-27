use crate::cmd::{Cli, Command};
use crate::display::{DevicePrinter, DisplayOpts};
use crate::r#trait::Execute;
use crate::util::get_devices;

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
            Command::DeChatter { timeouts } => {
                println!("{:?}", timeouts);
            }
        }

        Ok(())
    }
}
