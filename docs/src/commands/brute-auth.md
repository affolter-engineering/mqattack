# brute-auth

Test username/password combinations against the broker. The broker's ConnAck
return code (`BadUserNamePassword`, `NotAuthorized` or success) determines
whether a credential pair is valid.

## Synopsis

```bash
mqattack brute-auth [OPTIONS]
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `-c, --credentials <FILE>` | - | Credential pairs file (`username:password` per line) |
| `-u, --username <USER>` | - | Single username for dictionary attack |
| `-U, --user-list <FILE>` | - | Username wordlist |
| `-W, --pass-list <FILE>` | - | Password wordlist |
| `-P, --password <PASS>` | - | Single password for spray attack |
| `--try-anonymous` | off | Test anonymous (no-credentials) access first |
| `--stop-on-success` | off | Stop after the first successful login |
| `--delay <MS>` | `0` | Milliseconds between attempts |
| `--timeout <SECS>` | `5` | Seconds to wait per attempt |
| `--force` | off | Override the cross-product safety cap (10000 attempts) |

## Attack modes

| Mode | Required flags | Use case |
|------|---------------|---------|
| Credential pairs | `-c FILE` | Test a pre-built `username:password` list |
| Single-user brute-force | `-u USER -W FILE` | Dictionary attack on one account |
| Password spray | `-U FILE -P PASS` | One password against many users |
| Cross-product | `-U FILE -W FILE` | All combinations (capped at 10000) |

## Credential pairs file format

One `username:password` pair per line. The **first** colon is the separator,
so passwords may contain additional colons. Blank lines and lines starting
with `#` are ignored. A line with no colon is treated as a username with an
empty password.

```text
# credentials.txt
admin:admin
admin:password
guest:
service:pass:word:with:colons
```

## Examples

### Anonymous access probe, then credential list

```bash
mqattack brute-auth -H 192.168.1.10 \
  --try-anonymous -c credentials.txt --stop-on-success
```

### Dictionary attack on a single account

```bash
mqattack brute-auth -H 192.168.1.10 \
  -u admin -W /usr/share/wordlists/rockyou.txt
```

### Password spray with rate limiting

Using `--delay` to space attempts and avoid triggering account lockouts.

```bash
mqattack brute-auth -H 192.168.1.10 \
  -U users.txt -P 'Mqtt@2024' --delay 500
```

### Full cross-product over TLS

```bash
mqattack brute-auth --cafile ca.crt \
  -H broker.example.com -p 8883 \
  -U users.txt -W passwords.txt
```

### Cross-product exceeding the safety cap

```bash
mqattack brute-auth -H 192.168.1.10 \
  -U users.txt -W large-passwords.txt --force
```

### Stop on first success, pipe valid credentials to enum-perms

```bash
mqattack brute-auth -H 192.168.1.10 -c credentials.txt --stop-on-success \
  | grep 'SUCCESS' | awk '{print $2}' > valid.txt

mqattack enum-perms -H 192.168.1.10 -c valid.txt -w topics.txt
```

## Notes

- The cross-product mode (`-U FILE -W FILE`) is capped at 10,000 attempts by
  default to prevent accidental denial-of-service conditions.  Use `--force`
  to remove the cap.
- Add `--delay` when testing production systems or when the broker enforces
  rate limits or temporary bans.
- `--try-anonymous` is always the first check, regardless of other flags.
