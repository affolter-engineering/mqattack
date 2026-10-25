# check-acl

Probe subscribe and/or publish permissions for one topic or a list of topics.
Each test opens a fresh connection to the broker and inspects the SubAck or
PubAck returned.

## Synopsis

```bash
mqattack check-acl [OPTIONS] (-t <TOPIC> | -w <FILE>)
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `-t, --topic <TOPIC>` | - | Single topic to probe (exclusive with `-w`) |
| `-w, --wordlist <FILE>` | - | File with one topic per line (exclusive with `-t`) |
| `--no-subscribe` | off | Skip subscribe permission tests |
| `--no-publish` | off | Skip publish permission tests |
| `--payload <TEXT>` | `mqattack` | Payload used for publish probes |
| `-q, --qos <QOS>` | `1` | QoS for publish probes (1 or 2) |
| `--wait <SECS>` | `5` | Seconds to wait for a broker response per test |

## Result codes

| Result | Meaning |
|--------|---------|
| `ALLOWED` | Broker acknowledged with a success code |
| `DENIED` | Explicit denial or broker disconnected after publish |
| `INCONCLUSIVE` | No clear response received (QoS 0 publish, timeout) |
| `SKIPPED` | Test was not performed (e.g. `--no-publish`) |

Publish probes require QoS 1 or 2 to receive an acknowledgement.
QoS 0 always yields `INCONCLUSIVE` for publish tests because there is no
broker acknowledgement at that QoS level.

## Wordlist format

The wordlist file contains one topic per line. Blank lines and lines starting
with `#` are ignored.

```text
# topics.txt
home/lights/living-room
home/lights/bedroom
sensors/temperature
sensors/humidity
device/+/cmd
```

## Examples

### Test a single topic (subscribe and publish)

```bash
mqattack check-acl -H 192.168.1.10 -u admin -P secret -t 'home/lights/1'
```

### Test all topics from a wordlist, subscribe only

```bash
mqattack check-acl -H 192.168.1.10 -w topics.txt --no-publish
```

### Test publish permissions only with QoS 2

```bash
mqattack check-acl -H 192.168.1.10 -t 'cmd/device/1' --no-subscribe -q 2
```

### Check ACL over TLS with a custom CA

```bash
mqattack check-acl --cafile ca.crt \
  -H broker.example.com -p 8883 -w topics.txt
```

### Custom payload and longer wait window

Some brokers or network paths are slow. Increase `--wait` to avoid false
`INCONCLUSIVE` results.

```bash
mqattack check-acl -H 192.168.1.10 -t 'sensor/temp' \
  --payload 'test-payload' --wait 10
```

### Feed results into further analysis

```bash
mqattack check-acl -H 192.168.1.10 -w topics.txt \
  | grep ALLOWED | awk '{print $1}' > writable-topics.txt
```
