use std::ops::RangeInclusive;
use std::sync::Arc;
use std::time::Duration;

use clap::{Parser, Subcommand};

/// Debounce / de-chattering utility for key input devices.
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
        /// Show the path to the input device
        #[arg(short, long, default_value = "true")]
        path: bool,

        /// Show the physical path to the input device
        #[arg(short = 'P', long, default_value = "true")]
        physical_path: bool,

        /// Show the name of the input device
        #[arg(short, long, default_value = "false")]
        name: bool,

        /// Show the bus, vendor, product, and version of the input device
        #[arg(short, long, default_value = "false")]
        id: bool,

        /// Show all keys supported by the input device
        #[arg(short, long, default_value = "false")]
        keys: bool,

        /// Enable all flags: (-Ppnik)
        #[arg(short, long, default_value = "false")]
        all: bool,
    },

    /// Grab the device and de-chatter it.
    /// Filters support prefixes: s: starts with, e: ends with, c: contains, no prefix means equals.
    DeChatter {
        /// Inclusive ranges of keys to de-chatter, in format <start>:<end>:<timeout_ms> (repeatable)
        #[arg(short, long, value_parser = parse_key_range)]
        timeouts: Vec<KeyRangeTimeout>,

        /// Repeatable device name filter (Example: -n s:'Asus')
        #[arg(short, long, value_parser = parse_filter)]
        name: Vec<StringFilter>,

        /// Repeatable device path filter (Example: -p '/dev/input/event10')
        #[arg(short, long, value_parser = parse_filter)]
        path: Vec<StringFilter>, // TODO: path filter must be PathBuf/OsString, not String

        /// Repeatable device physical path filter (Example: -P 'usb-0000:04:00.3-3/input2')
        #[arg(short = 'P', long, value_parser = parse_filter)]
        physical_path: Vec<StringFilter>,

        /// Take the device with this index after applying all filters
        #[arg(short = 'i', long, default_value_t = 0)]
        index: usize,
    },
}

#[derive(Debug, Clone)]
pub(super) enum StringFilter {
    StartsWidth(Arc<str>),
    Equals(Arc<str>),
    Contains(Arc<str>),
    EndsWidth(Arc<str>),
}

impl StringFilter {
    pub(super) fn matches(&self, raw: impl AsRef<str>) -> bool {
        let raw = raw.as_ref();
        match self {
            StringFilter::StartsWidth(filter) => raw.starts_with(filter.as_ref()),
            StringFilter::Equals(filter) => raw == filter.as_ref(),
            StringFilter::Contains(filter) => raw.contains(filter.as_ref()),
            StringFilter::EndsWidth(filter) => raw.ends_with(filter.as_ref()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    if range.is_empty() {
        return Err(format!("Invalid empty range set for key range {}", raw));
    }

    let timeout = Duration::from_millis(parts[2] as u64);

    if timeout.as_millis() == 0 {
        return Err(format!("Invalid zero timeout set for key range {}", raw));
    }

    Ok(KeyRangeTimeout { range, timeout })
}

fn parse_filter(raw: &str) -> Result<StringFilter, String> {
    if let Some(raw) = raw.strip_prefix("s:") {
        Ok(StringFilter::StartsWidth(raw.into()))
    } else if let Some(raw) = raw.strip_prefix("e:") {
        Ok(StringFilter::EndsWidth(raw.into()))
    } else if let Some(raw) = raw.strip_prefix("c:") {
        Ok(StringFilter::Contains(raw.into()))
    } else {
        Ok(StringFilter::Equals(raw.into()))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_key_range() {
        use super::*;
        assert_eq!(
            parse_key_range("1:2:3").unwrap(),
            KeyRangeTimeout {
                range: 1..=2,
                timeout: Duration::from_millis(3),
            }
        );

        assert!(parse_key_range("1:2").is_err());
        assert!(parse_key_range("1:2:3:4").is_err());
        assert!(parse_key_range("0:0:0").is_err());
        assert!(parse_key_range("0:0:1").is_ok());
        assert!(parse_key_range("1:0:1").is_err());
    }
}
