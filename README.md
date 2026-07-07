# mqattack

A Rust-based MQTT penetration testing tool for the command line.

## Features

- **Subscribe** - listen to one or more topic filters
- **Publish** - send a payload to a topic
- **Check ACL** - probe subscribe/publish permissions for one topic or a wordlist
- **TLS** - broker verification with system or custom CA
- **mTLS** - mutual TLS with client certificate and private key
- **JWT authentication** - send a JWT token as the MQTT password
- **Insecure mode** - skip certificate verification for quick testing

## Installation

### From source (with Nix)

```bash
$ nix-shell
$ cargo build --release
```

The binary is placed at `target/release/mqattack`.

### From source (without Nix)

Requires Rust 1.80+, a C compiler, and OpenSSL development headers.

```bash
$ cargo build --release
```

## Usage

All commands share a common set of connection and authentication options:

```bash
Connection options:
  -H, --host <HOST>        Broker hostname or IP        [default: localhost]
  -p, --port <PORT>        Broker port                  [default: 1883]
  -u, --username <USER>    Username
  -P, --password <PASS>    Password (mutually exclusive with --jwt)
      --jwt <TOKEN>        JWT token sent as MQTT password
      --client-id <ID>     MQTT client identifier       [default: mqattack]
      --keepalive <SECS>   Keep-alive interval          [default: 60]

TLS options:
      --tls                Enable TLS with system root CAs
      --cafile <PATH>      Custom CA certificate in PEM format (implies --tls)
      --cert <PATH>        Client certificate for mTLS in PEM format (requires --key)
      --key <PATH>         Client private key for mTLS in PEM format (requires --cert)
      --insecure           Disable certificate verification — for testing only
```

### Subscribe

```bash
$ mqattack subscribe [OPTIONS] -t <TOPIC>...

  -t, --topic <TOPIC>    Topic filter, repeatable
  -q, --qos <QOS>        QoS level 0/1/2         [default: 0]
  -C, --count <N>        Exit after N messages
      --hex              Print payload as hexadecimal
```

```bash
# Subscribe to all topics
$ mqattack subscribe -t '#'

# Subscribe to multiple filters, exit after 5 messages
$ mqattack subscribe -t 'home/#' -t 'sensors/#' -C 5

# Plaintext with credentials
$ mqattack subscribe -H 192.168.1.10 -u admin -P secret -t '#'

# TLS using system root CAs (default port 8883)
$ mqattack subscribe --tls -H broker.example.com -p 8883 -t '#'

# TLS with a custom CA
$ mqattack subscribe --cafile ca.crt -H broker.example.com -p 8883 -t '#'

# mTLS with client certificate
$ mqattack subscribe --cert client.crt --key client.key --cafile ca.crt \
  -H broker.example.com -p 8883 -t '#'

# JWT authentication over TLS
$ mqattack subscribe --tls --jwt 'eyJhbGci...' -H broker.example.com -p 8883 -t '#'

# Self-signed cert — skip verification
$ mqattack subscribe --insecure -H 192.168.1.10 -p 8883 -t '#'

# Collect for 5 seconds, print a grouped summary
$ mqattack sys-info -H 192.168.1.10

# Longer collection window
$ mqattack sys-info -H broker.example.com --tls -p 8883 -w 15

# Stream values live
$ mqattack sys-info -H 192.168.1.10 --live
```

### Publish

```bash
$ mqattack publish [OPTIONS] -t <TOPIC> -m <MESSAGE>

  -t, --topic <TOPIC>    Target topic
  -m, --message <MSG>    Payload; use '-' to read from stdin
  -q, --qos <QOS>        QoS level 0/1/2         [default: 0]
  -r, --retain           Set the retain flag
```

```bash
# Publish a simple message
$ mqattack publish -t test/topic -m 'hello'

# Publish with QoS 1 and retain flag
$ mqattack publish -H 192.168.1.10 -t alerts/door -m 'open' -q 1 -r

# Pipe payload from stdin
$ echo '{"cmd":"reboot"}' | mqattack publish -t device/1/cmd -m -

# Publish over TLS with JWT
$ mqattack publish --tls --jwt 'eyJhbGci...' \
  -H broker.example.com -p 8883 -t device/1/cmd -m 'reboot'

# mTLS publish
$ mqattack publish --cert client.crt --key client.key --cafile ca.crt \
  -H broker.example.com -p 8883 -t test -m 'hello'
```

### Check ACL

Probe subscribe and/or publish permissions for one topic or a list of topics.
Each test connects fresh to the broker and inspects the SubAck/PubAck returned.
Results are reported as `ALLOWED`, `DENIED`, `INCONCLUSIVE` (no acknowledgement
received), or `SKIPPED`.

> **Note:** publish probes require QoS 1 or 2 (`-q 1` or `-q 2`) to get a
> broker acknowledgement. QoS 0 always yields `INCONCLUSIVE` for publish.

```bash
$ mqattack check-acl [OPTIONS] (-t <TOPIC> | -w <FILE>)

  -t, --topic <TOPIC>     Single topic to probe (mutually exclusive with --wordlist)
  -w, --wordlist <FILE>   File with one topic per line (mutually exclusive with --topic)
      --no-subscribe      Skip subscribe permission tests
      --no-publish        Skip publish permission tests
      --payload <TEXT>    Payload used for publish probes    [default: mqattack]
  -q, --qos <QOS>         QoS for publish probes (1 or 2)   [default: 1]
      --wait <SECS>       Seconds to wait for a broker response per test [default: 5]
```

```bash
# Test a single topic (both subscribe and publish)
$ mqattack check-acl -H 192.168.1.10 -u admin -P secret -t 'home/lights/1'

# Test all topics from a wordlist, subscribe only
$ mqattack check-acl -H 192.168.1.10 -w topics.txt --no-publish

# Test publish permissions only with QoS 2
$ mqattack check-acl -H 192.168.1.10 -t 'cmd/device/1' --no-subscribe -q 2

# Check ACL over TLS with a custom CA
$ mqattack check-acl --cafile ca.crt -H broker.example.com -p 8883 -w topics.txt

# Use a custom payload and a longer wait window
$ mqattack check-acl -H 192.168.1.10 -t 'sensor/temp' --payload 'test' --wait 10
```

### Enumerate topics

```bash
# Wordlist enumeration
mqattack enum-topics -H 192.168.1.10 -w mqtt-topics.txt

# Brute-force single-level topics under "home/"
mqattack enum-topics -H 192.168.1.10 --brute --prefix home --max-length 4

# Two-level brute-force (home/word/word) with longer window
mqattack enum-topics -H 192.168.1.10 --brute --prefix home --depth 2 -t 10
```

### Brute-force authentication

Test username/password combinations against the broker. Four attack modes are
supported, all using the broker's ConnAck return code to distinguish success
from failure (e.g. `BadUserNamePassword` vs. `NotAuthorized`).

| Mode | Flags | Use case |
|---|---|---|
| Credential pairs | `-c FILE` | Test a pre-built `username:password` list |
| Single-user brute-force | `-u USER -W FILE` | Dictionary attack on one account |
| Password spray | `-U FILE -P PASS` | One password against many users |
| Cross-product | `-U FILE -W FILE` | All combinations (capped at 10 000, override with `--force`) |

`--try-anonymous` probes a no-credentials connection before any wordlist attempt.

```bash
$ mqattack brute-auth [OPTIONS]

  -c, --credentials <FILE>   Credential pairs file (username:password per line)
  -U, --user-list <FILE>     Username wordlist
  -W, --pass-list <FILE>     Password wordlist
      --try-anonymous        Test anonymous access first
      --stop-on-success      Stop after the first successful login
      --delay <MS>           Milliseconds between attempts        [default: 0]
      --timeout <SECS>       Seconds to wait per attempt          [default: 5]
      --force                Override the cross-product safety cap
```

The credential pairs file uses `username:password` per line.
Blank lines and lines starting with `#` are ignored.
The first `:` is the separator; passwords may contain additional colons.

```text
# credentials.txt
admin:admin
admin:password
# no password
guest
# password contains a colon
service:pass:word
```

```bash
# Quick probe: test anonymous access then a credential list, stop on first hit
$ mqattack brute-auth -H 192.168.1.10 --try-anonymous -c credentials.txt --stop-on-success

# Dictionary attack on a single account
$ mqattack brute-auth -H 192.168.1.10 -u admin -W /usr/share/wordlists/rockyou.txt

# Password spray: one password across many users (with a 500 ms delay to avoid lockouts)
$ mqattack brute-auth -H 192.168.1.10 -U users.txt -P 'Mqtt@2024' --delay 500

# Full cross-product brute-force over TLS
$ mqattack brute-auth --cafile ca.crt -H broker.example.com -p 8883 \
  -U users.txt -W passwords.txt

# Pipe valid credentials directly into enum-perms to map their permissions
$ mqattack brute-auth -H 192.168.1.10 -c credentials.txt | grep 'SUCCESS' | \
  awk '{print $2}' > valid.txt && \
  mqattack enum-perms -H 192.168.1.10 -c valid.txt -w topics.txt
```

## License

MIT, see [LICENSE](LICENSE).
