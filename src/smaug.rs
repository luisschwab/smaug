use std::{collections::HashMap, thread, time::Duration};

use bitcoin::address::{Address, NetworkChecked};
use esplora_client::{Builder, Utxo};
use log::{debug, error, info};

use thiserror::Error;

use crate::Config;
use crate::check_addresses;
use crate::email::{EmailError, build_messages, send_messages};

/// The amount of seconds to sleep for between checks.
pub(crate) const SLEEP_SECS: u64 = 10;

/// A [`HashMap`] that maps an address to multiple [`Utxo`]s.
pub(crate) type UtxoDB = HashMap<Address<NetworkChecked>, Vec<Utxo>>;

#[derive(Clone, Debug)]
pub(crate) struct EventParams {
    /// What address this event refers to.
    pub(crate) address: Address,
    /// What [`UTXO`] this event refers to.
    pub(crate) utxo: Utxo,
    /// What height this event happened at.
    pub(crate) height: u32,
}

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
    /// Error parsing an [Address<NetworkUnchecked>] to an [Address<NetworkChecked>].
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
pub(crate) fn compute_diff(current_state: &Vec<Utxo>, last_state: &Vec<Utxo>) -> (Vec<Utxo>, Vec<Utxo>) {
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

pub(crate) fn handle_event(config: &Config, event: &Event) -> Result<(), SmaugError> {
    let messages = build_messages(config, event)?;

    // Send subscription and deposit emails
    // iff `notify_subscriptions` and `notify_deposits` are set.
    match event {
        Event::Subscription(_) => {
            if config.notify_deposits == true {
                send_messages(config, &messages)?;
            }
        }
        Event::Deposit(_) => {
            if config.notify_deposits == true {
                send_messages(config, &messages)?;
            }
        }
        Event::Withdrawal(_) => send_messages(config, &messages)?,
    }

    Ok(())
}

pub(crate) async fn smaug(config: &Config) -> Result<(), SmaugError> {
    // Build the esplora client `Smaug` will use to make requests.
    let esplora = Builder::new(&config.esplora_url).build_async()?;

    // Get the current chain tip.
    let mut current_chain_tip = esplora.get_height().await?;

    // Perform network validation on the addresses provided.
    let addresses = check_addresses(&config.addresses, &config.network)?;

    // Populate the [`UtxoDB`] with the initial state.
    let mut current_state = UtxoDB::new();
    for address in &addresses {
        // Fetch the UTXOs currently locked to the address.
        let utxos = esplora.get_address_utxos(&address).await?;

        info!("Subscribed to address {} at height {}", address, current_chain_tip);

        // Insert the address UTXOs into the UtxoDB.
        current_state.insert(address.clone(), utxos);
    }
    debug!("initial_state = {:#?}", current_state);

    // Send subscription email iff `config.notify_subscriptions` is set.
    if config.notify_subscriptions == true {
        let event = Event::Subscription(addresses.clone());
        handle_event(config, &event)?;
    }

    loop {
        // Fetch the current height.
        let last_chain_tip = current_chain_tip;
        current_chain_tip = esplora.get_height().await?;

        // Check if the `current_chain_tip` is superior than `last_chain_tip`. If not, skip.
        if current_chain_tip <= last_chain_tip {
            thread::sleep(Duration::from_secs(SLEEP_SECS));
            continue;
        }

        // The initial state becomes the last state.
        let last_state = current_state.clone();

        info!("Fetching state at height {}...", current_chain_tip);

        // Fetch the current state from Esplora.
        let mut current_state = UtxoDB::new();
        for address in &addresses {
            let utxos = esplora.get_address_utxos(&address).await?;
            current_state.insert(address.clone(), utxos);
        }

        // Compute the difference between states and generate [`Event`]s.
        let mut events: Vec<Event> = Vec::new();
        for address in &addresses {
            let (deposited, withdrawn) =
                compute_diff(current_state.get(address).unwrap(), last_state.get(address).unwrap());

            for deposit in deposited {
                let event: Event = Event::Deposit(EventParams {
                    address: address.clone(),
                    utxo: deposit,
                    height: current_chain_tip,
                });
                events.push(event);
            }
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
                Event::Deposit(_) => handle_event(config, &event)?,
                Event::Withdrawal(_) => handle_event(config, &event)?,
                _ => {}
            }
        }

        thread::sleep(Duration::from_secs(SLEEP_SECS));
    }
}
