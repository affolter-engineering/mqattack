# publish

Send a single payload to an MQTT topic.

## Synopsis

```bash
mqattack publish [OPTIONS] -t <TOPIC> -m <MESSAGE>
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `-t, --topic <TOPIC>` | *(required)* | Target topic |
| `-m, --message <MSG>` | *(required)* | Payload; pass `-` to read from stdin |
| `-q, --qos <QOS>` | `0` | QoS level: 0, 1 or 2 |
| `-r, --retain` | off | Set the MQTT retain flag |

When QoS 1 or 2 is used, `mqattack` waits for the broker acknowledgement
(PubAck / PubComp) before disconnecting.

Passing `-m -` reads the payload from standard input, allowing one to pipe
arbitrary data directly into a topic.

## Examples

### Publish a simple message

```bash
mqattack publish -H 192.168.1.10 -t test/topic -m 'hello'
```

### QoS 1 with retain flag

```bash
mqattack publish -H 192.168.1.10 -t alerts/door -m 'open' -q 1 -r
```

### Pipe a JSON payload from stdin

```bash
echo '{"cmd":"reboot"}' | mqattack publish -H 192.168.1.10 -t device/1/cmd -m -
```

### Pipe output of another command

```bash
sensor-read | mqattack publish -H 192.168.1.10 -t sensors/temp -m -
```

### Publish over TLS with JWT

```bash
mqattack publish --tls --jwt 'eyJhbGci...' \
  -H broker.example.com -p 8883 -t device/1/cmd -m 'reboot'
```

### mTLS publish

```bash
mqattack publish \
  --cafile ca.crt --cert client.crt --key client.key \
  -H broker.example.com -p 8883 -t test -m 'hello'
```

### Set a retained value (persists for future subscribers)

```bash
mqattack publish -H 192.168.1.10 -t config/timezone -m 'UTC' -r -q 1
```

### Clear a retained message

Publish an empty payload with the retain flag to clear a retained message:

```bash
mqattack publish -H 192.168.1.10 -t config/timezone -m '' -r -q 1
```
