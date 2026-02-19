use std::sync::LazyLock;

use clap::Parser;

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(long, default_value = "[::1]:4361")]
    pub address: String,

    #[arg(long)]
    pub directory: String,
}

pub static ARGS: LazyLock<Args> = LazyLock::new(Args::parse);
