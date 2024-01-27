use std::ops::RangeInclusive;
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
        /// Inclusive ranges of keys to de-chatter, in format <start>:<end>:<timeout_ms>
        #[arg(short, long, value_parser = parse_key_range)]
        timeouts: Vec<KeyRangeTimeout>,
    },
}

#[derive(Debug, Clone)]
pub(super) struct KeyRangeTimeout {
    range: RangeInclusive<u16>,
    timeout: Duration,
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
