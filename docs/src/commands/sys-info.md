# sys-info

Collect and display all `$SYS` broker metadata.  Unlike `fingerprint`, which
produces a summary focused on identification, `sys-info` shows the raw `$SYS`
topic tree in a readable grouped format.

## Synopsis

```bash
mqattack sys-info [OPTIONS]
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `-w, --wait <SECS>` | `5` | Collection window in seconds |
| `--live` | off | Stream messages as received instead of showing a grouped summary |

## Output format

**Default (summary mode):** Topics are grouped by their first three path
segments and printed as a table.  For example, all topics under
`$SYS/broker/clients/` are shown together.

**Live mode:** Each message is printed as it arrives:

```text
$SYS/broker/clients/connected       3
$SYS/broker/clients/total          47
$SYS/broker/messages/received    1082
```

## Examples

### Collect for 5 seconds, print grouped summary

```bash
mqattack sys-info -H 192.168.1.10
```

### Longer collection window

```bash
mqattack sys-info -H 192.168.1.10 -w 15
```

### Live stream

```bash
mqattack sys-info -H 192.168.1.10 --live
```

### sys-info over TLS

```bash
mqattack sys-info --tls -H broker.example.com -p 8883 -w 15
```

### Save all `$SYS` data to a file

```bash
mqattack sys-info -H 192.168.1.10 --live > sys-dump.txt
```

## Notes

- `$SYS` access is typically restricted to privileged users on production
  brokers.  A successful collection indicates a misconfiguration.
- The `live` mode is useful when you want to watch how statistics change over
  time (e.g., observing connected client counts during a brute-force run).
