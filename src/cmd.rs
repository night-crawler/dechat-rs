use std::ops::RangeInclusive;
use std::sync::Arc;
use std::time::Duration;

use clap::{Parser, Subcommand};

/// Debounce / de-chattering tool for input devices.
#[derive(Parser, Debug)]
#[command(author, version, about)]
#[command(propagate_version = true)]
pub(super) struct Cli {
    #[command(subcommand)]
    pub(super) command: Command,
}

#[derive(Subcommand, Debug)]
pub(super) enum Command {
    /// List all input devices
    List {
        /// Show path to input
        #[arg(short, long, default_value = "true")]
        path: bool,

        /// Show physical path
        #[arg(short = 'y', long, default_value = "true")]
        physical_path: bool,

        /// Show name
        #[arg(short, long, default_value = "false")]
        name: bool,

        /// Show bus, vendor, product, version
        #[arg(short, long, default_value = "false")]
        id: bool,

        /// Show all supported keys
        #[arg(short, long, default_value = "false")]
        keys: bool,

        /// Show everything
        #[arg(short, long, default_value = "false")]
        all: bool,
    },

    /// Grab the device and de-chatter it
    DeChatter {
        /// Inclusive ranges of keys to de-chatter, in format <start>:<end>:<timeout_ms> (repeatable)
        #[arg(short, long, value_parser = parse_key_range)]
        timeouts: Vec<KeyRangeTimeout>,

        /// Filter devices by name (repeatable)
        #[arg(short, long, value_parser = parse_filter)]
        name: Vec<Filter>,

        /// Filter devices by path (repeatable)
        #[arg(short, long, value_parser = parse_filter)]
        path: Vec<Filter>, // TODO: path filter must be PathBuf/OsString, not String

        /// Filter devices by physical path (repeatable)
        #[arg(short = 'y', long, value_parser = parse_filter)]
        physical_path: Vec<Filter>,

        /// Take the device with this index after applying all filters
        #[arg(short = 'i', long)]
        index: Option<usize>,
    },
}

#[derive(Debug, Clone)]
pub(super) enum Filter {
    StartsWidth(Arc<String>),
    Equals(Arc<String>),
    Contains(Arc<String>),
    EndsWidth(Arc<String>),
}

impl Filter {
    pub(super) fn matches(&self, raw: impl AsRef<str>) -> bool {
        let raw = raw.as_ref();
        match self {
            Filter::StartsWidth(filter) => raw.starts_with(filter.as_str()),
            Filter::Equals(filter) => raw == filter.as_str(),
            Filter::Contains(filter) => raw.contains(filter.as_str()),
            Filter::EndsWidth(filter) => raw.ends_with(filter.as_str()),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct KeyRangeTimeout {
    pub(super) range: RangeInclusive<u16>,
    pub(super) timeout: Duration,
}

fn parse_key_range(raw: &str) -> Result<KeyRangeTimeout, String> {
    let parts = raw.splitn(3, ':').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(format!("Invalid key range: {}", raw));
    }
    let parts = match parts
        .iter()
        .map(|part| part.parse::<u16>())
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(parts) => parts,
        Err(err) => return Err(format!("Invalid key range {raw}: {}", err)),
    };

    let range = parts[0]..=parts[1];
    let timeout = Duration::from_millis(parts[2] as u64);
    Ok(KeyRangeTimeout { range, timeout })
}

fn parse_filter(raw: &str) -> Result<Filter, String> {
    if let Some(raw) = raw.strip_prefix("s:") {
        Ok(Filter::StartsWidth(Arc::new(raw.to_string())))
    } else if let Some(raw) = raw.strip_prefix("e:") {
        Ok(Filter::EndsWidth(Arc::new(raw.to_string())))
    } else if let Some(raw) = raw.strip_prefix("c:") {
        Ok(Filter::Contains(Arc::new(raw.to_string())))
    } else {
        Ok(Filter::Equals(Arc::new(raw.to_string())))
    }
}
