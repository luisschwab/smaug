# Smaug

<p align="center">
  <img src="smaug.jpg" width="50%" alt="Smaug">
</p>

> Well, thief! I smell you, I hear your breath, I feel your air. Where are you?
>
> Come now, don't be shy... step into the light.
>
> There you are, Thief in the Shadows!

`smaug` is a tool that keeps watch of your UTXOs. If you subscribe to an address and UTXOs from it move,
`smaug` will send you an email to warn you.

## Use Case

Let's say you use [BIP85](https://bip85.com) to derive child seeds from a master seed,
and you only keep a physical backup of the master seed. It's critical to know if that seed has been
compromised (e.g.: an unauthorized person has access to it). If you deposit some bitcoin on an address
of the master seed and subscribe to that address, you'll be notified via email if that money ever moves.
If it does move, it means that your master seed has been compromised, giving you time to move your funds
from child seeds elsewhere before an attacker gets his hands on them.

## Usage

Install the Rust toolchain:
```shell
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Fix environment:
```shell
echo '. "$HOME/.cargo/env"' >> ~/.bashrc
source ~/.bashrc
```

Clone this repository and install the binary:
```
git clone https://github.com/luisschwab/smaug
cd smaug
cargo install --path .
```

Get these SMTP credentials from your provider (or from yourself, if you're into that kind of thing):
- SMTP username
- SMTP password
- SMTP server
- SMTP port

Create a TOML file with these fields:

```toml
# The netowrk to operate in: bitcoin, signet, testnet, testnet4
network = "testnet4"
# Optional: The URL of an Esplora API.
# If no Esplora API is defined, the Mempool.space API will be used by default.
esplora_url = "https://mempool.space/testnet4/api"
# A list of addresses to watch
addresses = [
    "tb1pk3su3yelyq4349c23rrmk0xa34dpmxght2t2ssenqj9vz9s4692shkkxxd",
    "tb1pp0aea5wv49f43t30hex2x5avlxelxlac7uwrjr0u57k7xnld3qzqnulr5q",
    "tb1punh3uhchgyaa0h95pxwyjkatn7qvulm6043gzfqwvmqw3f9vyetstd73va"
]
# Whether to send an email with the addresses you subscribed to
notify_subscriptions = true
# Whether to send an email about deposits to the addresses you subscribed to
notify_deposits = true
# The email addresses of notification recipients
recipient_emails = ["bilbo@baggins.net"]
# The SMTP username
smtp_username = "smaug@erebor.com"
# The SMTP password
smtp_password = "50m3r4nd0mp455w0rd"
# The SMTP server
smtp_server = "smtp.erebor.com"
# The SMTP port
smtp_port = 1337
```

Then run it (you should get an email about your subscribed addresses, if set):

```shell
smaug -c config.toml
[2025-09-11T04:50:35Z INFO  smaug] Successfully parsed configuration from `config.toml`
[2025-09-11T04:50:36Z INFO  smaug::smaug] Subscribed to address tb1pk3su3yelyq4349c23rrmk0xa34dpmxght2t2ssenqj9vz9s4692shkkxxd at height 101596
[2025-09-11T04:50:36Z INFO  smaug::smaug] Subscribed to address tb1pp0aea5wv49f43t30hex2x5avlxelxlac7uwrjr0u57k7xnld3qzqnulr5q at height 101596
[2025-09-11T04:50:36Z INFO  smaug::smaug] Subscribed to address tb1punh3uhchgyaa0h95pxwyjkatn7qvulm6043gzfqwvmqw3f9vyetstd73va at height 101596
[2025-09-11T04:50:41Z INFO  smaug::email] Sent email to bilbo@baggins.net
[2025-09-11T04:52:33Z INFO  smaug::smaug] Fetching state at height 101597...
[2025-09-11T05:12:34Z INFO  smaug::smaug] Fetching state at height 101598...
[2025-09-11T05:12:41Z INFO  smaug::smaug] Heads up, someone withdrew 1,000,000 sats from address tb1pk3su3yelyq4349c23rrmk0xa34dpmxght2t2ssenqj9vz9s4692shkkxxd
[2025-09-11T05:12:42Z INFO  smaug::smaug] Sent email to bilbo@baggins.net

```

__Note:__ Some cloud providers block SMTP ports 25, 465, and 587 by default. This is the case with [Digital Oceans](https://docs.digitalocean.com/support/why-is-smtp-blocked/), for example. To check this in advance, it is a good idea to test the SMTP connection to the gmail.com server.

```shell
timeout 8 openssl s_client -crlf -connect smtp.gmail.com:465 </dev/null
timeout 8 openssl s_client -starttls smtp -crlf -connect smtp.gmail.com:587 </dev/null
```

-> If a server certificate is displayed, the connection is possible.

Optionally, use the example [`systemd`](./smaug.service.example) service provided here:
```shell
cp smaug.service.example /etc/systemd/system/smaug.service
systemctl daemon-reload
systemctl enable smaug.service
systemctl start smaug.service
```

## Architecture

`smaug` is very simple: it hits the `/address/{address}/utxo` Esplora endpoint to get the current state
of the address (what UTXOs are locked to it). Then, it does long polling to the same endpoint
and computes the differences between the last state and the current state, here called `Event`s.
These get classified in `Event::Deposit` or `Event::Withdrawal`, `smaug` logs it and notifies
the recipients via email.
