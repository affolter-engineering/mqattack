# Connection & Authentication

Every mqattack command shares the same set of connection, authentication, and
TLS options.  These flags can be combined freely with any subcommand.

## Connection options

| Flag | Default | Description |
|------|---------|-------------|
| `-H, --host <HOST>` | `localhost` | Broker hostname or IP address |
| `-p, --port <PORT>` | `1883` | Broker port |
| `--client-id <ID>` | `mqattack` | MQTT client identifier |
| `--keepalive <SECS>` | `60` | Keep-alive interval in seconds |

## Authentication options

| Flag | Description |
|------|-------------|
| `-u, --username <USER>` | Username |
| `-P, --password <PASS>` | Password (mutually exclusive with `--jwt`) |
| `--jwt <TOKEN>` | JWT token sent as the MQTT password field |

`--jwt` and `--password` are mutually exclusive.

## TLS options

| Flag | Description |
|------|-------------|
| `--tls` | Enable TLS using the system root CA store |
| `--cafile <PATH>` | Custom CA certificate in PEM format (implies `--tls`) |
| `--cert <PATH>` | Client certificate for mTLS in PEM format (requires `--key`) |
| `--key <PATH>` | Client private key for mTLS in PEM format (requires `--cert`) |
| `--insecure` | Disable certificate verification - for lab use only |

## Authentication scenarios

### Anonymous access

```bash
mqattack subscribe -H 192.168.1.10 -t '#'
```

### Username and password

```bash
mqattack subscribe -H 192.168.1.10 -u admin -P secret -t '#'
```

### JWT token

```bash
mqattack subscribe -H broker.example.com -p 8883 --tls \
  --jwt 'eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9...' -t '#'
```

## TLS scenarios

### System root CAs (public broker)

```bash
mqattack subscribe --tls -H broker.example.com -p 8883 -t '#'
```

### Custom CA (self-signed or private PKI)

```bash
mqattack subscribe --cafile /path/to/ca.crt \
  -H broker.example.com -p 8883 -t '#'
```

### Mutual TLS (mTLS)

The broker requires a client certificate in addition to verifying the server.

```bash
mqattack subscribe \
  --cafile ca.crt \
  --cert client.crt \
  --key  client.key \
  -H broker.example.com -p 8883 -t '#'
```

The private key may be in PKCS#1, PKCS#8, or SEC1 (EC) format.

### Skip certificate verification

Useful for development environments with self-signed certificates where you
do not have the CA file.

```bash
mqattack subscribe --insecure -H 192.168.1.10 -p 8883 -t '#'
```

> **Warning:** `--insecure` disables all certificate checks.  Never use it
> against production systems.
