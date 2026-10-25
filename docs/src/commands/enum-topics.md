# enum-topics

Discover active MQTT topics on the broker by subscribing to candidate topic
strings in batches and observing which ones deliver messages.

## Synopsis

```bash
mqattack enum-topics [OPTIONS] (-w <FILE> | --brute)
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `-w, --wordlist <FILE>` | - | File with candidate topics, one per line |
| `--brute` | off | Generate candidates via character permutations |
| `--prefix <PREFIX>` | - | Prepend a fixed path segment before generated words |
| `--max-length <N>` | `4` | Maximum generated word length |
| `--depth <N>` | `1` | Number of generated levels (e.g., `2` -> `prefix/word/word`) |
| `--charset <CHARS>` | `a-z0-9_-` | Characters to use in brute-force generation |
| `-b, --batch-size <N>` | `200` | Subscriptions per batch |
| `-t, --timeout <SECS>` | `3` | Seconds to listen per batch |
| `-q, --qos <QOS>` | `0` | QoS level for subscriptions |
| `--force` | off | Override the 100000 candidate safety cap |

## How it works

Candidates are subscribed to in batches.  After each batch `mqattack` waits
`--timeout` seconds for any retained or live messages to arrive.  Topics that
return messages are reported as found.

Because only topics with current retained messages or active publishers are
discoverable, the results are a lower bound. Topics exist but may simply be
idle.

## Examples

### Wordlist enumeration

```bash
mqattack enum-topics -H 192.168.1.10 -w mqtt-topics.txt
```

### Brute-force single-level topics under a prefix

```bash
mqattack enum-topics -H 192.168.1.10 --brute --prefix home --max-length 4
```

### Two-level brute-force (`home/word/word`)

```bash
mqattack enum-topics -H 192.168.1.10 --brute --prefix home --depth 2 --timeout 10
```

### Custom charset (lowercase letters only)

```bash
mqattack enum-topics -H 192.168.1.10 --brute --prefix device \
  --charset abcdefghijklmnopqrstuvwxyz --max-length 6
```

### Authenticated enumeration

```bash
mqattack enum-topics -H 192.168.1.10 -u admin -P secret -w topics.txt
```

### Larger batches for faster scanning

```bash
mqattack enum-topics -H 192.168.1.10 -w large-wordlist.txt --batch-size 500
```

### Override the candidate safety cap

```bash
mqattack enum-topics -H 192.168.1.10 --brute --max-length 5 --force
```

## Notes

- The safety cap (100000 candidates) exists to prevent accidental memory
  exhaustion. Use `--force` only after estimating the candidate count.
- Brute-force generation grows exponentially with `--max-length` and `--depth`.
  Check the estimate printed before generation starts.
- Press Ctrl+C to abort an in-progress scan. Topics discovered so far are
  still reported.
