use crate::mqtt::client::{connect_and_subscribe, poll_loop, ConnectionArgs};
use anyhow::Result;
use clap::Args;
use rumqttc::QoS;
use std::collections::BTreeMap;
use tokio::signal;
use tokio::time::{timeout, Duration};

#[derive(Args, Debug)]
pub struct FingerprintArgs {
    #[command(flatten)]
    pub connection: ConnectionArgs,

    /// Seconds to collect $SYS messages before analysis
    #[arg(short = 'w', long, default_value_t = 5)]
    pub wait: u64,

    /// Print all collected $SYS messages after the summary
    #[arg(long)]
    pub verbose: bool,
}

#[derive(Debug, Default, PartialEq)]
enum Confidence {
    #[default]
    Unknown,
    Low,
    High,
}

impl std::fmt::Display for Confidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Confidence::Unknown => write!(f, "unknown"),
            Confidence::Low => write!(f, "low - $SYS accessible but broker not recognized"),
            Confidence::High => write!(f, "high"),
        }
    }
}

#[derive(Debug, Default)]
struct FingerprintResult {
    software: Option<String>,
    version: Option<String>,
    node_name: Option<String>,
    build_info: Option<String>,
    uptime: Option<String>,
    clients_connected: Option<String>,
    clients_total: Option<String>,
    msgs_received: Option<String>,
    msgs_sent: Option<String>,
    confidence: Confidence,
}

/// Run the fingerprint command: collect $SYS messages and analyze them to identify
/// the broker software and extract key metadata.
pub async fn run(args: FingerprintArgs) -> Result<()> {
    let mut conn = args.connection.clone();
    conn.client_id = format!("{}-fp", conn.client_id);

    eprintln!(
        "[*] Connecting to {}:{}",
        args.connection.host, args.connection.port
    );
    eprintln!("[*] Collecting $SYS data ({} second(s))...\n", args.wait);

    let (client, mut eventloop) =
        connect_and_subscribe(&conn, &["$SYS/#".to_string()], QoS::AtMostOnce).await?;

    let mut msgs: BTreeMap<String, String> = BTreeMap::new();

    tokio::select! {
        _ = timeout(Duration::from_secs(args.wait), poll_loop(&mut eventloop, |p| {
            let value = String::from_utf8_lossy(&p.payload).into_owned();
            msgs.insert(p.topic.clone(), value);
            true
        })) => {}
        _ = signal::ctrl_c() => {
            eprintln!("[*] Interrupted");
        }
    }

    client.disconnect().await.ok();

    if msgs.is_empty() {
        eprintln!("[!] No $SYS messages received.");
        eprintln!("[!] The broker may not publish $SYS data, or subscribe access is denied.");
        return Ok(());
    }

    let fp = analyze(&msgs);
    print_fingerprint(&fp, msgs.len());

    if args.verbose {
        println!("\n[*] All $SYS messages ({}):", msgs.len());
        println!("    {:<55} {}", "Topic", "Value");
        println!("    {}", "-".repeat(80));
        for (topic, value) in &msgs {
            println!("    {:<55} {}", topic, value);
        }
    } else {
        println!(
            "\n[*] {} $SYS message(s) collected (use --verbose to see all)",
            msgs.len()
        );
    }

    Ok(())
}

/// Print the fingerprint result in a human-readable format.
fn print_fingerprint(fp: &FingerprintResult, msg_count: usize) {
    println!("[+] Broker Fingerprint");
    println!(
        "    {:<22} {}",
        "Software:",
        fp.software.as_deref().unwrap_or("Unknown")
    );
    if let Some(v) = &fp.version {
        println!("    {:<22} {}", "Version:", v);
    }
    if let Some(n) = &fp.node_name {
        println!("    {:<22} {}", "Node:", n);
    }
    println!("    {:<22} {}", "Confidence:", fp.confidence);
    if let Some(b) = &fp.build_info {
        println!("    {:<22} {}", "Build info:", b);
    }

    // Only print the separator if there is at least one stat to show.
    let has_stats = [
        &fp.uptime,
        &fp.clients_connected,
        &fp.clients_total,
        &fp.msgs_received,
        &fp.msgs_sent,
    ]
    .iter()
    .any(|f| f.is_some());

    if has_stats {
        println!();
        row("Uptime:", &fp.uptime);
        row("Clients connected:", &fp.clients_connected);
        row("Clients total:", &fp.clients_total);
        row("Messages received:", &fp.msgs_received);
        row("Messages sent:", &fp.msgs_sent);
    }

    if msg_count > 0 && fp.confidence == Confidence::Unknown {
        println!();
        eprintln!(
            "[*] Broker not identified. Run with --verbose to inspect all {} collected topic(s).",
            msg_count
        );
    }
}

/// Helper function to print a label and an optional value in a formatted row.
fn row(label: &str, value: &Option<String>) {
    if let Some(v) = value {
        println!("    {:<22} {}", label, v);
    }
}

/// Identify the broker software and extract key metadata from collected $SYS messages.
fn analyze(msgs: &BTreeMap<String, String>) -> FingerprintResult {
    let mut fp = FingerprintResult::default();

    fp.uptime = get(msgs, "$SYS/broker/uptime");
    fp.clients_connected = get(msgs, "$SYS/broker/clients/connected")
        .or_else(|| get(msgs, "$SYS/broker/clients/active"));
    fp.clients_total = get(msgs, "$SYS/broker/clients/total")
        .or_else(|| get(msgs, "$SYS/broker/clients/maximum"));
    fp.msgs_received = get(msgs, "$SYS/broker/messages/received");
    fp.msgs_sent = get(msgs, "$SYS/broker/messages/sent");

    // Uses $SYS/brokers/<node>/... (plural) instead of $SYS/broker/...
    let emqx_keys: Vec<&String> = msgs
        .keys()
        .filter(|t| t.starts_with("$SYS/brokers/"))
        .collect();

    if !emqx_keys.is_empty() {
        fp.software = Some("EMQX".into());
        fp.confidence = Confidence::High;

        // Extract node name and version from $SYS/brokers/<node>/version
        if let Some(ver_key) = emqx_keys.iter().find(|t| t.ends_with("/version")) {
            let parts: Vec<&str> = ver_key.splitn(5, '/').collect();
            if parts.len() >= 3 {
                fp.node_name = Some(parts[2].to_owned());
            }
            fp.version = msgs.get(*ver_key).cloned();
        }
        if fp.uptime.is_none() {
            if let Some(up_key) = emqx_keys.iter().find(|t| t.ends_with("/uptime")) {
                fp.uptime = msgs.get(*up_key).cloned();
            }
        }
        return fp;
    }

    // Uses $SYS/nanomq/... namespace
    if msgs.keys().any(|t| t.starts_with("$SYS/nanomq/")) {
        fp.software = Some("NanoMQ".into());
        fp.confidence = Confidence::High;
        fp.version = get(msgs, "$SYS/nanomq/version");
        fp.uptime = get(msgs, "$SYS/nanomq/uptime")
            .or_else(|| get(msgs, "$SYS/nanomq/duration"));
        return fp;
    }

    // Mosquitto, HiveMQ, VerneMQ and RabbitMQ all expose $SYS/broker/version
    // but with distinct value prefixes.
    if let Some(ver) = get(msgs, "$SYS/broker/version") {
        fp.confidence = Confidence::High;

        if let Some(v) = ver.strip_prefix("mosquitto version ") {
            fp.software = Some("Mosquitto".into());
            fp.version = Some(v.to_owned());
            fp.build_info = get(msgs, "$SYS/broker/build-info");
        } else if let Some(v) = ver.strip_prefix("HiveMQ ") {
            fp.software = Some("HiveMQ".into());
            fp.version = Some(v.to_owned());
        } else if let Some(v) = ver.strip_prefix("VerneMQ ") {
            fp.software = Some("VerneMQ".into());
            fp.version = Some(v.to_owned());
        } else if ver.to_ascii_lowercase().contains("rabbitmq") {
            fp.software = Some("RabbitMQ".into());
            fp.version = Some(ver);
        } else {
            // $SYS/broker/version exists but format is not recognized.
            fp.version = Some(ver);
            fp.confidence = Confidence::Low;
        }
        return fp;
    }

    fp.confidence = Confidence::Unknown;
    fp
}

/// Helper function to get a value from the BTreeMap by key, returning an Option<String>.
fn get(msgs: &BTreeMap<String, String>, key: &str) -> Option<String> {
    msgs.get(key).cloned()
}

/// Unit tests for fingerprinting
#[cfg(test)]
mod tests {
    use super::*;

    fn msgs(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn fingerprint_mosquitto() {
        let m = msgs(&[("$SYS/broker/version", "mosquitto version 2.0.18")]);
        let fp = analyze(&m);
        assert_eq!(fp.software.as_deref(), Some("Mosquitto"));
        assert_eq!(fp.version.as_deref(), Some("2.0.18"));
        assert_eq!(fp.confidence, Confidence::High);
    }

    #[test]
    fn fingerprint_hivemq() {
        let m = msgs(&[("$SYS/broker/version", "HiveMQ 4.12.0")]);
        let fp = analyze(&m);
        assert_eq!(fp.software.as_deref(), Some("HiveMQ"));
        assert_eq!(fp.version.as_deref(), Some("4.12.0"));
        assert_eq!(fp.confidence, Confidence::High);
    }

    #[test]
    fn fingerprint_vernemq() {
        let m = msgs(&[("$SYS/broker/version", "VerneMQ 1.13.0")]);
        let fp = analyze(&m);
        assert_eq!(fp.software.as_deref(), Some("VerneMQ"));
        assert_eq!(fp.version.as_deref(), Some("1.13.0"));
        assert_eq!(fp.confidence, Confidence::High);
    }

    #[test]
    fn fingerprint_emqx_node_name() {
        let m = msgs(&[
            ("$SYS/brokers/emqx@localhost/version", "5.0.0"),
            ("$SYS/brokers/emqx@localhost/uptime", "12345 seconds"),
        ]);
        let fp = analyze(&m);
        assert_eq!(fp.software.as_deref(), Some("EMQX"));
        assert_eq!(fp.version.as_deref(), Some("5.0.0"));
        assert_eq!(fp.node_name.as_deref(), Some("emqx@localhost"));
        assert_eq!(fp.confidence, Confidence::High);
    }

    #[test]
    fn fingerprint_nanomq() {
        let m = msgs(&[("$SYS/nanomq/version", "0.18.0")]);
        let fp = analyze(&m);
        assert_eq!(fp.software.as_deref(), Some("NanoMQ"));
        assert_eq!(fp.version.as_deref(), Some("0.18.0"));
        assert_eq!(fp.confidence, Confidence::High);
    }

    #[test]
    fn fingerprint_rabbitmq() {
        let m = msgs(&[("$SYS/broker/version", "3.10.0-rabbitmq")]);
        let fp = analyze(&m);
        assert_eq!(fp.software.as_deref(), Some("RabbitMQ"));
        assert_eq!(fp.confidence, Confidence::High);
    }

    #[test]
    fn fingerprint_low_confidence_unknown_version() {
        let m = msgs(&[("$SYS/broker/version", "SomeBroker 9.9.9")]);
        let fp = analyze(&m);
        assert!(fp.software.is_none());
        assert_eq!(fp.version.as_deref(), Some("SomeBroker 9.9.9"));
        assert_eq!(fp.confidence, Confidence::Low);
    }

    #[test]
    fn fingerprint_empty_messages() {
        let fp = analyze(&BTreeMap::new());
        assert!(fp.software.is_none());
        assert_eq!(fp.confidence, Confidence::Unknown);
    }

    #[test]
    fn fingerprint_sys_without_version() {
        let m = msgs(&[("$SYS/broker/uptime", "999 seconds")]);
        let fp = analyze(&m);
        assert!(fp.software.is_none());
        assert_eq!(fp.confidence, Confidence::Unknown);
        assert_eq!(fp.uptime.as_deref(), Some("999 seconds"));
    }
}
