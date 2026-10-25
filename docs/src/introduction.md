# Introduction

**mqattack** is a command-line MQTT penetration testing tool written in Rust.
It covers the full assessment lifecycle for MQTT brokers: from passive
reconnaissance and fingerprinting through credential attacks, ACL enumeration
and active payload injection.

## What is MQTT?

MQTT (Message Queuing Telemetry Transport) is a lightweight publish/subscribe
protocol widely used in IoT, industrial automation, home automation and
telemetry systems. Brokers such as Mosquitto, HiveMQ, EMQX, VerneMQ, NanoMQ
and RabbitMQ act as message hubs; clients publish messages to *topics* and
subscribe to topic filters to receive them.

Because MQTT devices are often resource-constrained and deployed in trusted
networks, brokers are commonly misconfigured:

- anonymous access enabled
- weak credentials accepted
- overly broad ACLs that allow any client to read or write any topic

## What mqattack does

| Command | Category | Purpose |
|---------|----------|---------|
| `subscribe` | Recon | Listen to one or more topic filters |
| `publish` | Interaction | Send a payload to a topic |
| `fingerprint` | Recon | Identify broker software and version via `$SYS` |
| `sys-info` | Recon | Collect all `$SYS` broker metadata |
| `check-acl` | Enumeration | Probe subscribe/publish permissions per topic |
| `brute-auth` | Attack | Brute-force or spray MQTT credentials |
| `enum-topics` | Enumeration | Discover active topics via wordlist or brute-force |
| `enum-perms` | Enumeration | Build a user * topic permission matrix |
| `will-inject` | Attack | Register a crafted Last Will to inject messages on disconnect |
| `payload-inject` | Attack | Publish built-in or custom injection payloads to a topic |

## Supported brokers

mqattack has been tested against:

- **Eclipse Mosquitto**
- **HiveMQ**
- **NanoMQ**
- **RabbitMQ** (with MQTT plugin)
- **Home Assistant** (with Mosquitto broker Add-ons)

Any MQTT 3.1.1-compliant broker should work. MQTT 5 extensions are not required.

## Scope and responsible use

mqattack is intended for **authorized security assessments**. Penetration
tests, CTF challenges, lab environments and research on infrastructure you own
or have explicit permission to test. Running it against systems without
authorization is illegal and unethical.

## License

mqattack is released under the **MIT License**.
See [LICENSE](https://github.com/affolter-engineering/mqattack/blob/main/LICENSE).
