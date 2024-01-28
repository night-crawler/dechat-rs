use clap::Parser;
use env_logger::Env;

use crate::cmd::Cli;
use crate::traits::Execute;

mod cmd;
mod dechatter;
mod display;
mod execute;
mod key_state;
mod traits;
mod util;

fn main() -> anyhow::Result<()> {
    let args = Cli::try_parse()?;
    let env = Env::default()
        .filter_or("LOG_LEVEL", "info")
        .write_style_or("LOG_STYLE", "always");

    env_logger::init_from_env(env);

    args.execute()?;
    Ok(())
}
