#![allow(unused)]

use std::{fs, process};

use bitcoin::{
    Network,
    address::{Address, NetworkUnchecked},
};
use clap::Parser;
use log::{debug, error, info};

use crate::error::SmaugError;
use crate::smaug::{Config, smaug};

mod error;
mod smaug;

/// TOML configuration file path CLI argument.
#[derive(Parser)]
#[command(name = "smaug")]
#[command(about = "smaug watches your addresses and sends you an email if they move")]
pub(crate) struct Cli {
    #[arg(long = "config", short = 'c', help = "The path to the TOML configuration file")]
    pub(crate) config: String,
}

fn parse_config(config_path: &str) -> Config {
    let config_str = match fs::read_to_string(&config_path) {
        Ok(config_str) => config_str,
        Err(_) => {
            error!("Failed to open `{config_path}`. Does the file exist?");
            process::exit(1);
        }
    };
    let config: Config = match toml::from_str(&config_str) {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to parse TOML from `{config_path}`: {e}");
            process::exit(1);
        }
    };
    info!("Successfully parsed configuration from `{config_path}`");

    debug!("");
    debug!("[smaug]");
    debug!("network = {}", config.network);
    debug!("esplora_url = {}", config.esplora_url);
    debug!("addresses = {:#?}", config.addresses);
    debug!("notify_deposits = {}", config.notify_deposits);
    debug!("");

    config
}

/// Check that the addresses and network provided are a match.
pub(crate) fn check_addresses(
    addresses: &Vec<Address<NetworkUnchecked>>,
    network: &Network,
) -> Result<Vec<Address>, SmaugError> {
    addresses
        .iter()
        .map(|addr| {
            addr.clone()
                .require_network(network.to_owned())
                .map_err(|e| SmaugError::NetworkMismatch(e))
        })
        .collect()
}

#[tokio::main]
async fn main() -> Result<(), SmaugError> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

    let args = Cli::parse();
    let config = parse_config(&args.config);

    let _ = smaug(config).await?;

    Ok(())
}
