use lettre::{
    Message, SmtpTransport, Transport,
    address::AddressError,
    error::Error as LettreError,
    message::{Mailbox, header::ContentType},
    transport::smtp::{
        self,
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
};
use log::{debug, info, warn};
use thiserror::Error;

use crate::Config;
use crate::format_with_commas;
use crate::smaug::Event;

/// Errors that happens while sending an email.
#[derive(Error, Debug)]
pub enum EmailError {
    /// TLS error.
    #[error(transparent)]
    TlsError(#[from] smtp::Error),

    /// Email address parsing error.
    #[error(transparent)]
    EmailAddressParsingError(#[from] AddressError),

    /// Email building error.
    #[error(transparent)]
    EmailBuildError(#[from] LettreError),
}

/// Create an email message from an [`Event`] to every address in `recipient_emails`.
pub(crate) fn build_messages(config: &Config, event: &Event) -> Result<Vec<Message>, EmailError> {
    // The sender's mailbox.
    let sender_mailbox = Mailbox::new(
        Some(String::from("Smaug, the UTXO guardian")),
        config.smtp_username.clone(),
    );

    // All the recipients we must build messages to.
    let recipient_mailboxes: Vec<Mailbox> = config
        .recipient_emails
        .iter()
        .map(|email| Mailbox::new(None, email.clone()))
        .collect();
    debug!("recipient_mailboxes: {:#?}", recipient_mailboxes);

    let (subject, body) = match event {
        Event::Subscription(addresses) => {
            let num_addresses = addresses.len();

            let subject: String = match num_addresses {
                1 => format!("You're now subscribed to 1 address"),
                _ => format!("You're now subscribed to {} addresses", num_addresses),
            };

            let mut body: String = match num_addresses {
                1 => String::from("You are now subscribed to this address:"),
                _ => String::from("You're now subscribed to these addresses:"),
            };
            for address in addresses {
                body.push_str(&format!("\n- {}", address));
            }

            debug!("Event::Subscription email:");
            debug!(" Subject: {subject}");
            debug!(" Body: {body}");

            (subject, body)
        }
        Event::Deposit(event_params) => {
            let subject = String::from("Someone deposited to an address you're subscribed to");

            let body = format!(
                "Someone deposited {} sats to address {}",
                format_with_commas(event_params.utxo.value.to_sat()),
                event_params.address
            );

            info!(
                "Someone deposited {} sats to address {} at height {}",
                format_with_commas(event_params.utxo.value.to_sat()),
                event_params.address,
                event_params.height
            );

            debug!("Event::Deposit email:");
            debug!(" Subject: {subject}");
            debug!(" Body: {body}");

            (subject, body)
        }
        Event::Withdrawal(event_params) => {
            let subject = String::from("Heads up, someone withdrew from an address you're subscribed to!");

            let body = format!(
                "Someone withdrew {} sats from address {}",
                format_with_commas(event_params.utxo.value.to_sat()),
                event_params.address
            );

            warn!(
                "Heads up, someone withdrew {} sats from address {} at height {}!",
                format_with_commas(event_params.utxo.value.to_sat()),
                event_params.address,
                event_params.height
            );

            debug!("Event::Withdrawal email:");
            debug!(" Subject: {subject}");
            debug!(" Body: {body}");

            (subject, body)
        }
    };

    let messages: Vec<Message> = recipient_mailboxes
        .iter()
        .map(|mailbox| {
            let message = Message::builder()
                .from(sender_mailbox.clone())
                .to(mailbox.clone())
                .subject(subject.clone())
                .header(ContentType::TEXT_PLAIN)
                .body(body.clone())
                .unwrap();
            message
        })
        .collect();

    Ok(messages)
}

/// Send email messages.
pub(crate) fn send_messages(config: &Config, messages: &Vec<Message>) -> Result<(), EmailError> {
    let smtp_credentials = Credentials::new(config.smtp_username.to_string(), config.smtp_password.clone());
    let tls = TlsParameters::new_rustls(config.smtp_server.clone())?;
    let mailer = SmtpTransport::relay(&config.smtp_server)?
        .port(config.smtp_port)
        .credentials(smtp_credentials)
        .tls(Tls::Required(tls))
        .build();

    debug!("Sending {} emails...", messages.len());
    for message in messages {
        mailer.send(&message)?;
        if let Some(recipient) = message.envelope().to().first() {
            info!("Sent email to {}", recipient);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use bitcoin::{Address, Amount, Network, Txid};
    use esplora_client::{Utxo, UtxoStatus};

    use super::*;
    use crate::parse_config;
    use crate::smaug::{Event, EventParams};

    #[test]
    fn build_and_send_email() {
        let _ = env_logger::try_init();

        let config: Config = parse_config("config.toml");

        let address = Address::from_str("bc1qc86e5rpn2f2m6d76tzeq7hmz53cx08hqw8uhl7")
            .unwrap()
            .require_network(Network::Bitcoin)
            .unwrap();

        let event: Event = Event::Deposit(EventParams {
            address: address,
            utxo: Utxo {
                txid: Txid::from_str("33aeb7af5ff454dbbdc65c8229b13b2c101978976df655ae43ab8d467b5c8b9e").unwrap(),
                vout: 0,
                status: UtxoStatus {
                    confirmed: false,
                    block_height: None,
                    block_hash: None,
                    block_time: None,
                },
                value: Amount::from_sat(1337),
            },
            height: 900009,
        });

        let messages = build_messages(&config, &event).unwrap();

        println!("messages: {:#?}", messages);

        let _ = send_messages(&config, &messages).unwrap();
    }
}
