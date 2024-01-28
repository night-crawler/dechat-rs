use clap::Parser;
use env_logger::Env;

use crate::cmd::Cli;
use crate::traits::Execute;

mod cmd;
mod device_wrapper;
mod display;
mod execute;
mod key_filter;
mod key_state;
mod traits;

fn main() -> anyhow::Result<()> {
    let env = Env::default()
        .filter_or("LOG_LEVEL", "info")
        .write_style_or("LOG_STYLE", "always");
    env_logger::init_from_env(env);

    let args = Cli::try_parse()?;
    args.execute()?;
    Ok(())
}
