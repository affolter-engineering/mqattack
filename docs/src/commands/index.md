# Commands

`mqattack` provides ten subcommands grouped by purpose:

## Reconnaissance

| Command | Description |
|---------|-------------|
| [`subscribe`](./subscribe.md) | Listen to one or more topic filters |
| [`fingerprint`](./fingerprint.md) | Identify broker software and version via `$SYS` |
| [`sys-info`](./sys-info.md) | Collect all `$SYS` broker metadata |

## Interaction

| Command | Description |
|---------|-------------|
| [`publish`](./publish.md) | Send a payload to a topic |

## Enumeration

| Command | Description |
|---------|-------------|
| [`check-acl`](./check-acl.md) | Probe subscribe/publish permissions per topic |
| [`enum-topics`](./enum-topics.md) | Discover active topics via wordlist or brute-force |
| [`enum-perms`](./enum-perms.md) | Build a user × topic permission matrix |

## Attacks

| Command | Description |
|---------|-------------|
| [`brute-auth`](./brute-auth.md) | Brute-force or spray MQTT credentials |
| [`will-inject`](./will-inject.md) | Register a crafted Last Will to inject messages on disconnect |
| [`payload-inject`](./payload-inject.md) | Publish built-in or custom injection payloads |

All commands accept the [common connection and authentication flags](../connection.md).
