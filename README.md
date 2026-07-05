# mqattack

A Rust-based MQTT penetration testing tool for the command line.

## Features

- **Subscribe** - listen to one or more topic filters
- **Publish** - send a payload to a topic

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

### Subscribe

```bash
$ mqattack subscribe [OPTIONS] -t <TOPIC>...

Options:
  -H, --host <HOST>          Broker hostname or IP  [default: localhost]
  -p, --port <PORT>          Broker port            [default: 1883]
  -u, --username <USERNAME>  Username
  -P, --password <PASSWORD>  Password
  -t, --topic <TOPIC>        Topic filter (repeatable)
  -q, --qos <QOS>            QoS level 0/1/2        [default: 0]
  -C, --count <N>            Exit after N messages
      --hex                  Print payload as hexadecimal
      --client-id <ID>       Client identifier      [default: mqattack]
```

#### Examples

```bash
# Subscribe to all topics
$ mqattack subscribe -t '#'

# Subscribe to two filters, exit after 5 messages
$ mqattack subscribe -t 'home/#' -t 'sensors/#' -C 5

# Connect with credentials
$ mqattack subscribe -H 192.168.1.10 -u admin -P secret -t '#'
```

### Publish

```bash
$ mqattack publish [OPTIONS] -t <TOPIC> -m <MESSAGE>

Options:
  -H, --host <HOST>          Broker hostname or IP  [default: localhost]
  -p, --port <PORT>          Broker port            [default: 1883]
  -u, --username <USERNAME>  Username
  -P, --password <PASSWORD>  Password
  -t, --topic <TOPIC>        Target topic
  -m, --message <MSG>        Payload ('-' reads from stdin)
  -q, --qos <QOS>            QoS level 0/1/2        [default: 0]
  -r, --retain               Set the retain flag
      --client-id <ID>       Client identifier      [default: mqattack]
```

#### Examples

```bash
# Publish a simple message
$ mqattack publish -t test/topic -m 'hello'

# Publish with QoS 1 and retain flag
$ mqattack publish -H 192.168.1.10 -t alerts/door -m 'open' -q 1 -r

# Pipe payload from stdin
echo '{"cmd":"reboot"}' | mqattack publish -t device/1/cmd -m -
```

## License

MIT, see [LICENSE](LICENSE).
