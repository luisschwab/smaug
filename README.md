<p align="center">
  <img src="smaug.jpg" width="50%" alt="Smaug">
</p>

# Smaug

> Well, thief! I smell you, I hear your breath, I feel your air. Where are you?
>
> Come now, don't be shy... step into the light.

`smaug` is a tool that keeps watch of your UTXOs. If you subscribe to an address and UTXOs from it move, `smaug` will send you an email to warn you.

## Usage

Install the Rust toolchain:
```shell
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
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
# The URL of an esplora API
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
# The email addresses of recipients of notifications
recipient_emails = ["bilbo@example.org"]
# The SMTP username
smtp_username = "smaug@example.org"
# The SMTP password
smtp_password = "50m3r4nd0mp455w0rd"
# The SMTP server
smtp_server = "smtp.example.org"
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
[2025-09-11T04:50:41Z INFO  smaug::email] Sent email to bilbo@example.org
[2025-09-11T04:52:33Z INFO  smaug::smaug] Fetching state at height 101597...
[2025-09-11T05:12:34Z INFO  smaug::smaug] Fetching state at height 101598...
```

Optionally, use the example [`systemd`](./smaug.service.example) service provided here:
```shell
cp smaug.service.example /etc/systemd/system/smaug.service
systemctl daemon-reload
systemctl enable smaug.service
systemctl start smaug.service
```

## Architecture

`smaug` is very simple: it hits the `/address/{address}/utxo` Esplora endpoint to get the current state of the addresses
(i.e.: what UTXOs are currently locked to it). Then, it does long polling to the same endpoint, always computing the
difference of the last state and the current state. If a difference is computed, it get's classified as a `Deposit` or `Withdrawal`,
logs it and notifies the recipients via email.
