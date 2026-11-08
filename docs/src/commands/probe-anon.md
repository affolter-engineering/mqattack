# probe-anon

Single-shot check for anonymous (unauthenticated) broker access. First confirms
whether the broker accepts a connection with no credentials, then optionally
probes subscribe and publish permissions on a topic or wordlist — identical ACL
checks to [`check-acl`](./check-acl.md) but always without credentials.

Any `-u`/`-P`/`--jwt` flags passed alongside this command are ignored and a
warning is printed.

## Synopsis

```bash
mqattack probe-anon [OPTIONS]
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

Topics are optional. Without `-t` or `-w` the command only verifies the
anonymous CONNECT and exits.

## How it works

1. A CONNECT packet is sent with no username, password, or JWT token.
2. If the broker returns a successful ConnAck, anonymous access is confirmed.
3. If the broker refuses the connection (or the attempt times out), the command
   reports denial and exits.
4. When topics are provided, each one is tested for subscribe and publish
   permissions using fresh connections, in the same way as `check-acl`.

## Result codes

| Result | Meaning |
|--------|---------|
| `ALLOWED` | Broker acknowledged with a success code |
| `DENIED` | Explicit denial or broker disconnected after the probe |
| `INCONCLUSIVE` | No clear response received (QoS 0 publish, timeout) |
| `SKIPPED` | Test was not performed (e.g. `--no-publish`) |

Publish probes require QoS 1 or 2 to receive an acknowledgement.
QoS 0 always yields `INCONCLUSIVE` for publish tests.

## Examples

### Quick connectivity check — no topic probing

```bash
mqattack probe-anon -H 192.168.1.10
```

### Check anonymous connectivity and probe a single topic

```bash
mqattack probe-anon -H 192.168.1.10 -t '#'
```

### Probe a full topic wordlist

```bash
mqattack probe-anon -H 192.168.1.10 -w topics.txt
```

### Subscribe-only probe over TLS

```bash
mqattack probe-anon --cafile ca.crt \
  -H broker.example.com -p 8883 \
  -w topics.txt --no-publish
```

### Pipe writable topics into further analysis

```bash
mqattack probe-anon -H 192.168.1.10 -w topics.txt \
  | grep ALLOWED | awk '{print $1}' > anon-writable.txt
```

## Example output

```text
[*] Target : 192.168.1.10:1883
[+] Anonymous access ALLOWED

[*] 3 topic(s) | subscribe=true | publish=true

    home/lights/1                                       sub=ALLOWED       pub=ALLOWED
    home/alarm/status                                   sub=ALLOWED       pub=DENIED
    $SYS/broker/version                                 sub=INCONCLUSIVE  pub=DENIED

[+] Subscribe : 2 allowed, 0 denied, 1 inconclusive
[+] Publish   : 1 allowed, 2 denied, 0 inconclusive

[+] Topics with subscribe ALLOWED:
    home/lights/1
    home/alarm/status

[+] Topics with publish ALLOWED:
    home/lights/1
```

## Notes

- When no topics are provided the command exits immediately after the
  connectivity check, making it a fast one-liner for a first-pass assessment.
- Credentials passed via `-u`/`-P`/`--jwt` are stripped before the connection
  attempt; this guards against accidentally confirming access that relies on
  credentials copied from another command.
- For authenticated ACL probing see [`check-acl`](./check-acl.md) and
  [`enum-perms`](./enum-perms.md).
