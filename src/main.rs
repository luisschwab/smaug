use std::{fs, process};

use argh::FromArgs;
use bitcoin::{
    Network,
    address::{Address, NetworkUnchecked},
};
use lettre::Address as EmailAddress;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};

use crate::smaug::{SmaugError, smaug};

mod email;
mod smaug;

/// smaug watches your addresses and sends you an email if they move
#[derive(FromArgs)]
struct Cli {
    /// the path to the TOML configuration file
    #[argh(option, short = 'c')]
    config: String,
}

/// `smaug` configuration parameters.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Config {
    /// The network this program will operate on.
    pub(crate) network: Network,
    /// The full URL of the Esplora chain-source.
    pub(crate) esplora_url: String,
    /// The list of addresses to watch for movement.
    pub(crate) addresses: Vec<Address<NetworkUnchecked>>,
    /// Wheter to notify of address subscriptions (this will run once, at startup).
    pub(crate) notify_subscriptions: bool,
    /// Whether to notify of deposits to any of the addresses.
    pub(crate) notify_deposits: bool,
    /// Recipient emails for address notifications.
    pub(crate) recipient_emails: Vec<EmailAddress>,
    /// The SMTP username.
    pub(crate) smtp_username: EmailAddress,
    /// The SMTP password.
    pub(crate) smtp_password: String,
    /// The SMTP server.
    pub(crate) smtp_server: String,
    /// The SMTP port.
    pub(crate) smtp_port: u16,
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
    debug!("notify_subscriptions = {:#?}", config.notify_subscriptions);
    debug!("notify_deposits = {}", config.notify_deposits);
    debug!("recipient_emails = {:#?}", config.recipient_emails);
    debug!("smtp_username = {}", config.smtp_username);
    debug!("smtp_password = {}", config.smtp_password);
    debug!("smtp_server = {}", config.smtp_server);
    debug!("smtp_port = {}", config.smtp_port);
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

fn format_with_commas(num: u64) -> String {
    let num_str = num.to_string();
    let mut result = String::new();

    for (i, ch) in num_str.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }

    result.chars().rev().collect()
}

#[tokio::main]
async fn main() -> Result<(), SmaugError> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

    let args: Cli = argh::from_env();
    let config = parse_config(&args.config);

    let _ = smaug(&config).await?;

    Ok(())
}
