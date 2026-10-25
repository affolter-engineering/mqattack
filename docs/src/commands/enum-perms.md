# enum-perms

Build a user * topic permission matrix by testing subscribe and publish
permissions for every combination of credential and topic.

## Synopsis

```bash
mqattack enum-perms [OPTIONS] -c <FILE> (-t <TOPIC> | -w <FILE>)
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `-c, --credentials <FILE>` | *(required)* | Credential pairs file (`username:password` per line) |
| `-t, --topic <TOPIC>` | - | Single topic to test (exclusive with `-w`) |
| `-w, --wordlist <FILE>` | - | File with one topic per line (exclusive with `-t`) |

The credentials file uses the same format as `brute-auth`: one
`username:password` pair per line, first colon as separator, blank lines and
`#` comments ignored.

## Output format

Results are displayed as a matrix.  Each cell shows `S/P` where:

- `S` is the subscribe result: `+` (allowed), `-` (denied), `?` (inconclusive)
- `P` is the publish result: `+` (allowed), `-` (denied), `?` (inconclusive)

```text
Topic                   admin       sensor01    guest
home/lights/living      +/+         -/+         -/-
home/lights/bedroom     +/+         -/+         -/-
sensors/temperature     +/+         +/+         ?/?
sensors/humidity        +/+         +/+         ?/?
device/1/cmd            +/+         -/-         -/-
```

## Examples

### Test one topic against all users in a credential file

```bash
mqattack enum-perms -H 192.168.1.10 -c valid-creds.txt -t 'sensors/temperature'
```

### Full matrix: multiple users × topic wordlist

```bash
mqattack enum-perms -H 192.168.1.10 -c valid-creds.txt -w topics.txt
```

### Pipe valid credentials from brute-auth directly into enum-perms

```bash
mqattack brute-auth -H 192.168.1.10 -c candidates.txt \
  | grep 'SUCCESS' | awk '{print $2}' > valid.txt

mqattack enum-perms -H 192.168.1.10 -c valid.txt -w topics.txt
```

### enum-perms over TLS

```bash
mqattack enum-perms --cafile ca.crt \
  -H broker.example.com -p 8883 -c creds.txt -w topics.txt
```

## Notes

- Each user-topic pair requires two connections (subscribe and publish), so
  runtime scales with `users x topics x 2`.
- Publish results use QoS 1 internally. QoS 0 would always yield `?`.
- The matrix provides a quick overview of ACL policy. Use `check-acl` for a
  single-user deep-dive with more control over timeouts and payloads.
