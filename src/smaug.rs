use std::{collections::HashMap, process, thread, time::Duration};

use bitcoin::{
    Network,
    address::{Address, NetworkChecked},
};
use esplora_client::{BlockingClient, Builder, Utxo};
use log::{debug, error, info, warn};

use thiserror::Error;

use crate::Config;
use crate::check_addresses;
use crate::email::{EmailError, build_messages, send_messages};

/// The amount of seconds to sleep for between checks.
pub(crate) const POLLING_PERIOD_SEC: u64 = 30;

/// The amount of seconds to wait before retrying after an Esplora error.
pub(crate) const ERROR_RETRY_DELAY_SEC: u64 = 30;

/// A [`HashMap`] that maps an address to multiple [`Utxo`]s.
pub(crate) type UtxoDB = HashMap<Address<NetworkChecked>, Vec<Utxo>>;

/// Default Esplora API base URLs.
pub(crate) const BITCOIN_ESPLORA: &str = "https://mempool.space/api";
/// Signet Mempool.space Esplora API base URL.
pub(crate) const SIGNET_ESPLORA: &str = "https://mempool.space/signet/api";
/// Testnet4 Mempool.space Esplora API base URL.
pub(crate) const TESTNET4_ESPLORA: &str = "https://mempool.space/testnet4/api";

/// Parameters of an [`Event`] of kind `Subscription` or `Deposit`.
#[derive(Clone, Debug)]
pub(crate) struct EventParams {
    /// What address this event refers to.
    pub(crate) address: Address,
    /// What [`UTXO`] this event refers to.
    pub(crate) utxo: Utxo,
    /// What height this event happened at.
    pub(crate) height: u32,
}

/// An [`Event`] about a Bitcoin address.
#[derive(Clone, Debug)]
pub(crate) enum Event {
    /// Subscription to a set of addresses.
    Subscription(Vec<Address>),
    /// A deposit to an address.
    Deposit(EventParams),
    /// A withdrawal from an address.
    Withdrawal(EventParams),
}

#[derive(Debug, Error)]
pub(crate) enum SmaugError {
    /// Error parsing an [`Address<NetworkUnchecked>`] to an [`Address<NetworkChecked>`].
    #[error(transparent)]
    NetworkMismatch(#[from] bitcoin::address::ParseError),

    /// Error creating `EsploraClient`.
    #[error(transparent)]
    EsploraClient(#[from] esplora_client::Error),

    /// Error sending email notifications.
    #[error(transparent)]
    Email(#[from] EmailError),
}

/// Compute the difference in the set of UTXOs locked to an address.
///
/// Returns two vectors: deposited UTXOs and withdrawn UTXOs.
pub(crate) fn compute_diff(current_state: &[Utxo], last_state: &[Utxo]) -> (Vec<Utxo>, Vec<Utxo>) {
    let deposited: Vec<Utxo> = current_state
        .iter()
        .filter(|utxo| !last_state.contains(utxo))
        .cloned()
        .collect();

    let withdrawn: Vec<Utxo> = last_state
        .iter()
        .filter(|utxo| !current_state.contains(utxo))
        .cloned()
        .collect();

    (deposited, withdrawn)
}

/// Handle an [`Event`] according to it's variant.
pub(crate) fn handle_event(config: &Config, event: &Event) -> Result<(), SmaugError> {
    let messages = build_messages(config, event)?;

    // Send subscription and deposit emails
    // iff `notify_subscriptions` and `notify_deposits` are set.
    match event {
        Event::Subscription(_) => {
            if config.notify_subscriptions {
                send_messages(config, &messages)?;
            }
        }
        Event::Deposit(_) => {
            if config.notify_deposits {
                send_messages(config, &messages)?;
            }
        }
        Event::Withdrawal(_) => send_messages(config, &messages)?,
    }

    Ok(())
}

/// Fetch UTXOs for all addresses with retry logic.
fn fetch_utxos_with_retry(
    esplora: &BlockingClient,
    addresses: &[Address<NetworkChecked>],
) -> Result<UtxoDB, SmaugError> {
    let mut db = UtxoDB::new();

    for address in addresses {
        let utxos = esplora.get_address_utxos(address)?;
        db.insert(address.clone(), utxos);
    }

    Ok(db)
}

/// Long-poll the Esplora API, compute address state diffs, and notify the recipients if there is a diff.
pub(crate) fn smaug(config: &Config) -> Result<(), SmaugError> {
    let base_url = match &config.esplora_url {
        Some(url) => {
            info!("Using configured Esplora API: {url}");
            url
        }
        None => match &config.network {
            Network::Bitcoin => {
                info!("Using default Bitcoin Esplora API: {BITCOIN_ESPLORA}");
                BITCOIN_ESPLORA
            }
            Network::Signet => {
                info!("Using default Signet Esplora API: {SIGNET_ESPLORA}");
                SIGNET_ESPLORA
            }
            Network::Testnet4 => {
                info!("Using default Testnet4 Esplora API: {TESTNET4_ESPLORA}");
                TESTNET4_ESPLORA
            }
            _ => {
                error!("Other networks are not supported");
                process::exit(1);
            }
        },
    };

    // Build the esplora client `smaug` will use to make requests.
    let esplora = Builder::new(base_url).build_blocking();

    // Get the current chain tip with retry.
    let mut current_chain_tip = loop {
        match esplora.get_height() {
            Ok(height) => break height,
            Err(e) => {
                error!("Failed to fetch initial chain tip: {e}");
                error!("Retrying in {ERROR_RETRY_DELAY_SEC} seconds...");
                thread::sleep(Duration::from_secs(ERROR_RETRY_DELAY_SEC));
            }
        }
    };

    // Perform network validation on the provided [`Address`]es against the configured [`Network`].
    let addresses = check_addresses(&config.addresses, &config.network)?;

    // Populate the [`UtxoDB`] with the initial state with retry logic.
    let mut current_state = loop {
        match fetch_utxos_with_retry(&esplora, &addresses) {
            Ok(state) => {
                for address in &addresses {
                    info!("Subscribed to address {} at height {}", address, current_chain_tip);
                }
                debug!("initial_state = {:#?}", state);
                break state;
            }
            Err(e) => {
                error!("Failed to fetch initial UTXOs: {e}");
                error!("Retrying in {ERROR_RETRY_DELAY_SEC} seconds...");
                thread::sleep(Duration::from_secs(ERROR_RETRY_DELAY_SEC));
            }
        }
    };

    // Send subscription email iff `config.notify_subscriptions` is set.
    if config.notify_subscriptions {
        let event = Event::Subscription(addresses.clone());
        if let Err(e) = handle_event(config, &event) {
            warn!("Failed to send subscription notification: {e}");
        }
    }

    // Event Loop.
    loop {
        // Fetch the current height.
        let last_chain_tip = current_chain_tip;
        current_chain_tip = match esplora.get_height() {
            Ok(height) => height,
            Err(e) => {
                error!("Failed to fetch initial UTXOs: {e}");
                error!("Retrying in {ERROR_RETRY_DELAY_SEC} seconds...");
                thread::sleep(Duration::from_secs(ERROR_RETRY_DELAY_SEC));
                continue;
            }
        };

        // Check if the `current_chain_tip` is superior than `last_chain_tip`. If not, skip.
        if current_chain_tip <= last_chain_tip {
            thread::sleep(Duration::from_secs(POLLING_PERIOD_SEC));
            continue;
        }

        // The initial state becomes the last state.
        let last_state = current_state.clone();

        info!("Fetching state at height {}...", current_chain_tip);

        // Fetch the current state from Esplora with error handling.
        current_state = match fetch_utxos_with_retry(&esplora, &addresses) {
            Ok(state) => state,
            Err(e) => {
                warn!("Failed to fetch UTXOs: {e}");
                warn!("Keeping previous state and retrying in {ERROR_RETRY_DELAY_SEC} seconds...");
                thread::sleep(Duration::from_secs(ERROR_RETRY_DELAY_SEC));
                continue;
            }
        };

        // Compute the difference between states and generate [`Event`]s.
        let mut events: Vec<Event> = Vec::new();
        for address in &addresses {
            let (deposited, withdrawn) =
                compute_diff(current_state.get(address).unwrap(), last_state.get(address).unwrap());

            // Create [`Event::Deposit`]s based on the `UtxoDBs` diff between the last and current states.
            for deposit in deposited {
                let event: Event = Event::Deposit(EventParams {
                    address: address.clone(),
                    utxo: deposit,
                    height: current_chain_tip,
                });
                events.push(event);
            }

            // Create [`Event::Withdrawal`]s based on the `UtxoDBs` diff between the last and current states.
            for withdrawal in withdrawn {
                let event: Event = Event::Withdrawal(EventParams {
                    address: address.clone(),
                    utxo: withdrawal,
                    height: current_chain_tip,
                });
                events.push(event);
            }
        }
        debug!("events = {:#?}", events);

        for event in &events {
            match event {
                Event::Deposit(_) | Event::Withdrawal(_) => {
                    if let Err(e) = handle_event(config, event) {
                        warn!("Failed to handle event: {e}");
                    }
                }
                _ => unreachable!(),
            }
        }

        thread::sleep(Duration::from_secs(POLLING_PERIOD_SEC));
    }
}
