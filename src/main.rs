use clap::Parser as _;
use env_logger::Env;

use crate::cmd::Cli;
use crate::traits::Execute as _;

mod cmd;
mod debouncer;
mod device_wrapper;
mod display;
mod execute;
mod key_filter;
mod key_state;
mod traits;

fn main() -> anyhow::Result<()> {
    let env = Env::default()
        .filter_or("LOG_LEVEL", "info")
        .write_style_or("LOG_STYLE", "auto");
    env_logger::init_from_env(env);

    match Cli::try_parse() {
        Ok(args) => {
            args.execute()?;
        }
        Err(err) => {
            err.exit();
        }
    }

    Ok(())
}
