use colored::Colorize;

use crate::cmd::Command;
use crate::util::DeviceWrapper;

trait Colorizer<T> {
    fn write_arg(&mut self, key: &str, value: T) -> std::io::Result<()>;
}

impl<Q, T: std::fmt::Display> Colorizer<T> for Q
where
    Q: std::io::Write,
{
    fn write_arg(&mut self, key: &str, value: T) -> std::io::Result<()> {
        write!(self, "{}={}", key.blue(), value.to_string().magenta())
    }
}

pub(super) struct DisplayOpts {
    path: bool,
    physical_path: bool,
    name: bool,
    id: bool,
    keys: bool,
}

impl TryFrom<Command> for DisplayOpts {
    type Error = anyhow::Error;

    fn try_from(value: Command) -> Result<Self, Self::Error> {
        match value {
            Command::List {
                path,
                physical_path,
                name,
                id,
                keys,
                all,
            } => Ok(Self {
                path: path || all,
                physical_path: physical_path || all,
                name: name || all,
                id: id || all,
                keys: keys || all,
            }),
            Command::DeChatter { .. } => {
                anyhow::bail!("Can't construct DisplayOpts from anything byt List")
            }
        }
    }
}

pub(super) struct DevicePrinter<'a> {
    wrapper: &'a DeviceWrapper,
    display_opts: &'a DisplayOpts,
}

impl<'a> DevicePrinter<'a> {
    pub(super) fn new(
        wrapper: &'a DeviceWrapper,
        display_opts: &'a DisplayOpts,
    ) -> DevicePrinter<'a> {
        Self {
            wrapper,
            display_opts,
        }
    }

    pub(super) fn print(&self, f: &mut impl std::io::Write) -> std::io::Result<()> {
        let device = &self.wrapper.device;
        let path = &self.wrapper.path;
        if self.display_opts.path {
            f.write_arg("path", path.display())?;
        }

        if self.display_opts.physical_path {
            f.write_arg(
                " physical_path",
                device.physical_path().unwrap_or("No path"),
            )?;
        }

        if self.display_opts.name {
            f.write_arg(" name", device.name().unwrap_or("Unnamed device"))?;
        }

        if self.display_opts.id {
            let input_id = device.input_id();
            f.write_arg(" bus", input_id.bus_type())?;
            f.write_arg(" bus_id", format!("{:#x}", input_id.bus_type().0))?;
            f.write_arg(" vendor", format!("{:#x}", input_id.vendor()))?;
            f.write_arg(" product", format!("{:#x}", input_id.product()))?;
            f.write_arg(" version", format!("{:#x}", input_id.version()))?;
        }

        if self.display_opts.keys {
            let mut parts = device
                .supported_keys()
                .into_iter()
                .flat_map(|attribute_set| attribute_set.iter())
                .peekable();

            write!(f, "\n\t{}: ", "Keys".magenta())?;

            let has_keys = parts.peek().is_some();
            if !has_keys {
                writeln!(f, "{}", "None".red())?;
            }

            while let Some(key_code) = parts.next() {
                write!(
                    f,
                    "{}={}",
                    format!("{:?}", key_code).bold().blue(),
                    key_code.code().to_string().cyan()
                )?;
                if parts.peek().is_some() {
                    write!(f, ", ")?;
                }
            }

            if has_keys {
                writeln!(f)?;
            }
        } else {
            writeln!(f)?;
        }
        Ok(())
    }
}
