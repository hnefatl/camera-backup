use std::sync::LazyLock;

use clap::{ArgAction, Parser};

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(long)]
    pub source_root: String,

    #[arg(long)]
    pub server_address: String,

    #[arg(long, default_value_t=50)]
    pub max_inflight_sends: usize,
    #[arg(long, default_value_t=2 * 1024 * 1024)]
    pub chunk_size: usize,

    #[arg(long, default_value_t = true, action=ArgAction::Set)]
    pub dry_run: bool,

    #[arg(long, default_value_t = true, action=ArgAction::Set)]
    /// Use `libnotify` to send desktop notifications.
    pub send_notifications: bool,
}

pub static ARGS: LazyLock<Args> = LazyLock::new(Args::parse);
