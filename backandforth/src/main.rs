extern crate gstreamer as gst;

use anyhow::{Result,Error};
use std::env::{args,var};
use std::process::exit;

fn main() -> Result<()> {
    match args().iter().first() {
        Some("client") => client::run(),
        Some("server") => server::run(),
        None => Err(Error::msg("please specify {client,server} mode")),
    }
}

mod client;
mod server;
