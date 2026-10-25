# payload-inject

Publish built-in injection payloads or a custom wordlist to a topic and
optionally monitor a second topic for responses.

## Synopsis

```bash
mqattack payload-inject [OPTIONS] -t <TOPIC>
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `-t, --topic <TOPIC>` | *(required)* | Target topic |
| `--set <SET>` | - | Built-in payload set, repeatable (see table below) |
| `-w, --wordlist <FILE>` | - | Custom payload file (one payload per line) |
| `-q, --qos <QOS>` | `0` | QoS level: 0, 1, or 2 |
| `-r, --retain` | off | Set the retain flag on each published payload |
| `--delay <MS>` | `100` | Milliseconds between payloads |
| `--monitor <TOPIC>` | - | Subscribe on a second connection to capture responses |
| `--monitor-wait <SECS>` | `5` | Seconds to listen after the last payload |

## Built-in payload sets

| Set | Count | Contents |
|-----|-------|----------|
| `sql` | 13 | Classic, union, time-based, and error-based SQL injection |
| `cmd` | 15 | Shell operators (`;`, `\|`, `&&`), command substitution, cloud metadata endpoint |
| `xss` | 11 | Script tags, event handlers, template expressions (`{{7*7}}`, `${7*7}`) |
| `fmt` | 11 | Format string specifiers (`%x`, `%s`, `%n`) and Python object inspection |
| `xxe` | 5 | XML external entity file inclusion (`/etc/passwd`, Windows paths, XInclude) |
| `json` | 10 | NoSQL injection, prototype pollution, deeply nested structures |
| `mqtt` | 11 | Empty payload, null bytes, CRLF injection, wildcards, path traversal, malformed UTF-8, large payloads (1 KB, 64 KB) |
| `all` | 76 | All of the above combined |

Multiple `--set` flags can be combined; duplicates are deduplicated
automatically.

## Examples

### SQL injection probes

```bash
mqattack payload-inject -H 192.168.1.10 -t 'device/cmd' --set sql
```

### Combined SQL and command injection with response capture

```bash
mqattack payload-inject -H 192.168.1.10 \
  -t 'device/cmd' --set sql --set cmd \
  --monitor 'device/response' --monitor-wait 10
```

### All built-in payloads, slower delivery

```bash
mqattack payload-inject -H 192.168.1.10 \
  -t 'app/input' --set all --delay 500
```

### Custom payload wordlist

```bash
mqattack payload-inject -H 192.168.1.10 \
  -t 'device/cmd' -w my-payloads.txt
```

### Combined built-in and custom payloads

```bash
mqattack payload-inject -H 192.168.1.10 \
  -t 'device/cmd' --set sql -w extra-sqli.txt
```

### MQTT-specific fault injection (null bytes, oversized messages)

```bash
mqattack payload-inject -H 192.168.1.10 \
  -t 'app/data' --set mqtt --monitor 'app/data'
```

### Retain payloads so they persist for future subscribers

```bash
mqattack payload-inject -H 192.168.1.10 \
  -t 'config/value' --set json -r -q 1
```

### Injection over TLS with credentials

```bash
mqattack payload-inject --cafile ca.crt \
  -H broker.example.com -p 8883 -u writer -P secret \
  -t 'app/input' --set all
```

## Notes

- The monitor connection listens on a **separate** MQTT client; it can capture
  any topic including the target topic itself.
- `--delay 0` sends payloads back-to-back.  Increase it if the broker or the
  downstream application needs time to process each message.
- QoS 0 (`--qos 0`) gives maximum throughput; QoS 1 or 2 ensures each payload
  is acknowledged before the next is sent.
- `--set all` sends 76 payloads; budget `76 × delay` milliseconds plus monitor
  wait time for the full run.
