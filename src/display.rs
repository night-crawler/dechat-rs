use colored::Colorize as _;

use crate::cmd::Command;
use crate::device_wrapper::DeviceWrapper;

struct SpaceSeparatedPairs<'a, W> {
    out: &'a mut W,
    is_empty: bool,
}

impl<W: std::io::Write> SpaceSeparatedPairs<'_, W> {
    fn write_pair(&mut self, key: &str, value: impl std::fmt::Display) -> std::io::Result<()> {
        let separator = if self.is_empty { "" } else { " " };
        self.is_empty = false;
        write!(self.out, "{separator}{}={}", key.blue(), value.to_string().magenta())
    }
}

pub struct DisplayOpts {
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
            } => {
                // With only `-k`, the keys would be printed with no device next to them.
                // So the paths are shown unless the user asked for a name or an id.
                let is_anonymous = !(path || physical_path || name || id);
                Ok(Self {
                    path: path || all || is_anonymous,
                    physical_path: physical_path || all || is_anonymous,
                    name: name || all,
                    id: id || all,
                    keys: keys || all,
                })
            }
            Command::DeChatter { .. } => {
                anyhow::bail!("Can't construct DisplayOpts from anything but List")
            }
        }
    }
}

pub struct DevicePrinter<'a> {
    wrapper: &'a DeviceWrapper,
    display_opts: &'a DisplayOpts,
}

impl<'a> DevicePrinter<'a> {
    pub const fn new(wrapper: &'a DeviceWrapper, display_opts: &'a DisplayOpts) -> Self {
        Self { wrapper, display_opts }
    }

    pub fn print(&self, f: &mut impl std::io::Write) -> std::io::Result<()> {
        let device = &self.wrapper.device;
        let mut pairs = SpaceSeparatedPairs {
            out: &mut *f,
            is_empty: true,
        };

        if self.display_opts.path {
            pairs.write_pair("path", self.wrapper.path.display())?;
        }

        if self.display_opts.physical_path {
            pairs.write_pair("physical_path", device.physical_path().unwrap_or("?"))?;
        }

        if self.display_opts.name {
            pairs.write_pair("name", device.name().unwrap_or("?"))?;
        }

        if self.display_opts.id {
            let input_id = device.input_id();
            pairs.write_pair("bus", input_id.bus_type())?;
            pairs.write_pair("bus_id", format_args!("{:#x}", input_id.bus_type().0))?;
            pairs.write_pair("vendor", format_args!("{:#x}", input_id.vendor()))?;
            pairs.write_pair("product", format_args!("{:#x}", input_id.product()))?;
            pairs.write_pair("version", format_args!("{:#x}", input_id.version()))?;
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
