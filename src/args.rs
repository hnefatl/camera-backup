use std::sync::LazyLock;

use clap::{ArgAction, Parser};

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(long)]
    pub source_root: String,
    #[arg(long)]
    pub destination_root: String,
    #[arg(long, default_value_t = true, action=ArgAction::Set)]
    pub dry_run: bool,
    #[arg(long, default_value_t = 1000)]
    pub queue_capacity: usize,
}

pub static ARGS: LazyLock<Args> = LazyLock::new(|| Args::parse());
