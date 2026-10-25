# fingerprint

Identify the broker software and version by subscribing to `$SYS/#` and
analyzing the collected messages. No packets beyond a standard SUBSCRIBE are
sent. The broker reveals itself through what it publishes.

## Synopsis

```bash
mqattack fingerprint [OPTIONS]
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `-w, --wait <SECS>` | `5` | Collection window in seconds |
| `--verbose` | off | Print all collected `$SYS` messages after the summary |

## Recognized brokers

| Broker | Detection signal |
|--------|-----------------|
| Mosquitto | `$SYS/broker/version` starts with `mosquitto version` |
| HiveMQ | `$SYS/broker/version` starts with `HiveMQ` |
| VerneMQ | `$SYS/broker/version` starts with `VerneMQ` |
| EMQX | Topics under `$SYS/brokers/` (plural); node name extracted from path |
| NanoMQ | Topics under `$SYS/nanomq/` namespace |
| RabbitMQ | `$SYS/broker/version` contains `rabbitmq` |

Confidence levels:

- **high**: a known broker signature matched
- **low**: `$SYS/broker/version` is present but does not match any known broker
- **unknown**: `$SYS` is accessible but yields no identifying information

## Example output

```text
[+] Broker Fingerprint
    Software:              Mosquitto
    Version:               2.0.18
    Confidence:            high
    Build info:            With OpenSSL 3.0.2 15 Mar 2022

    Uptime:                12345 seconds
    Clients connected:     3
    Clients total:         47
    Messages received:     1082
    Messages sent:         2341

[*] 47 $SYS message(s) collected (use --verbose to see all)
```

## Examples

### Fingerprint a local broker

```bash
mqattack fingerprint -H 192.168.1.10
```

### Longer collection window for slow brokers

Some brokers only publish `$SYS` statistics at a low frequency. Increase the
wait window if the first attempt returns `unknown`.

```bash
mqattack fingerprint -H 192.168.1.10 -w 10
```

### Dump all collected `$SYS` messages

```bash
mqattack fingerprint -H 192.168.1.10 -w 10 --verbose
```

### Fingerprint over TLS with credentials

```bash
mqattack fingerprint --cafile ca.crt \
  -H broker.example.com -p 8883 -u admin -P secret
```

### Fingerprint with insecure TLS (self-signed cert)

```bash
mqattack fingerprint --insecure -H 192.168.1.10 -p 8883
```

## Notes

- If `$SYS` is disabled on the broker, fingerprinting will return `unknown`
  regardless of the wait window.
- EMQX clusters publish `$SYS/brokers/<node>/` paths; the node name is extracted
  automatically from the topic hierarchy.
- The `--verbose` flag is useful for discovering broker-specific `$SYS` paths
  that are not part of the standard specification.
