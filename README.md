# mqattack

A Rust-based MQTT penetration testing tool for the command line.

## Features

- **Subscribe** - listen to one or more topic filters
- **Publish** - send a payload to a topic
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

#### Examples

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

#### Examples

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

### Enumerate topics

```bash
# Wordlist enumeration
mqattack enum-topics -H 192.168.1.10 -w mqtt-topics.txt

# Brute-force single-level topics under "home/"
mqattack enum-topics -H 192.168.1.10 --brute --prefix home --max-length 4

# Two-level brute-force (home/word/word) with longer window
mqattack enum-topics -H 192.168.1.10 --brute --prefix home --depth 2 -t 10
```


## License

MIT, see [LICENSE](LICENSE).
