# subscribe

Listen to one or more MQTT topic filters and print received messages.

## Synopsis

```bash
mqattack subscribe [OPTIONS] -t <TOPIC>...
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `-t, --topic <TOPIC>` | *(required)* | Topic filter, repeatable |
| `-q, --qos <QOS>` | `0` | QoS level: 0, 1, or 2 |
| `-C, --count <N>` | *(unlimited)* | Exit after receiving N messages |
| `--hex` | off | Print payload bytes as hexadecimal |

Multiple `-t` flags can be combined to subscribe to several filters in one
connection.

## Output format

Each received message is printed on one line:

```text
[2026-10-25 14:32:01] home/sensors/temp  -> 22.4
```

With `--hex`:

```text
[2026-10-25 14:32:01] home/sensors/temp  -> 32 32 2e 34
```

## Examples

### Subscribe to all topics

```bash
mqattack subscribe -H 192.168.1.10 -t '#'
```

### Multiple topic filters, exit after 5 messages

```bash
mqattack subscribe -H 192.168.1.10 -t 'home/#' -t 'sensors/#' -C 5
```

### Authenticated subscription

```bash
mqattack subscribe -H 192.168.1.10 -u admin -P secret -t '#'
```

### TLS with system root CAs

```bash
mqattack subscribe --tls -H broker.example.com -p 8883 -t '#'
```

### TLS with a custom CA

```bash
mqattack subscribe --cafile ca.crt -H broker.example.com -p 8883 -t '#'
```

### mTLS (client certificate)

```bash
mqattack subscribe \
  --cafile ca.crt --cert client.crt --key client.key \
  -H broker.example.com -p 8883 -t '#'
```

### JWT authentication over TLS

```bash
mqattack subscribe --tls --jwt 'eyJhbGci...' \
  -H broker.example.com -p 8883 -t '#'
```

### Inspect binary payloads as hex

```bash
mqattack subscribe -H 192.168.1.10 -t 'device/raw' --hex
```

### Capture a fixed number of messages for offline analysis

```bash
mqattack subscribe -H 192.168.1.10 -t '#' -C 100 > capture.txt
```
