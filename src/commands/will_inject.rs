use crate::mqtt::client::{build_options, parse_qos, ConnectionArgs};
use anyhow::{Context, Result};
use clap::Args;
use rumqttc::{AsyncClient, Event, LastWill, Packet, QoS};
use tokio::time::{sleep, Duration};

#[derive(Args, Debug)]
pub struct WillInjectArgs {
    #[command(flatten)]
    pub connection: ConnectionArgs,

    /// Topic the broker publishes the will payload to on ungraceful disconnect
    #[arg(short = 't', long)]
    pub topic: String,

    /// Will payload
    #[arg(short = 'm', long, default_value = "mqattack-will")]
    pub message: String,

    /// QoS for the will message (0, 1, or 2)
    #[arg(short = 'q', long, default_value_t = 1)]
    pub qos: u8,

    /// Set the retain flag - will persists on the topic after delivery
    #[arg(short = 'r', long)]
    pub retain: bool,

    /// Seconds to hold the connection open before triggering the will
    #[arg(long, default_value_t = 2)]
    pub hold: u64,

    /// Subscribe to this topic on a second connection to capture the will being delivered
    #[arg(long)]
    pub monitor: Option<String>,

    /// Seconds to listen on the monitor topic after disconnect
    #[arg(long, default_value_t = 5)]
    pub monitor_wait: u64,
}

/// Run the "will-inject" command: register a will, then disconnect ungracefully.
pub async fn run(args: WillInjectArgs) -> Result<()> {
    let qos = parse_qos(args.qos)?;

    eprintln!("[*] Will topic  : {}", args.topic);
    eprintln!("[*] Will payload: {}", args.message);
    eprintln!("[*] Will QoS    : {}", args.qos);
    eprintln!("[*] Will retain : {}", args.retain);
    if let Some(m) = &args.monitor {
        eprintln!("[*] Monitor     : {}", m);
    }
    eprintln!();

    // Optional monitor connection — start before registering the will so we
    // don't miss the delivery window.
    let monitor_handle = if let Some(monitor_topic) = args.monitor.clone() {
        let mut conn = args.connection.clone();
        conn.client_id = format!("{}-monitor", conn.client_id);
        Some(tokio::spawn(run_monitor(conn, monitor_topic)))
    } else {
        None
    };

    // Build MQTT options with the will embedded in the CONNECT packet.
    let mut opts = build_options(&args.connection)?;
    let will = LastWill::new(
        &args.topic,
        args.message.as_bytes().to_vec(),
        qos,
        args.retain,
    );
    opts.set_last_will(will);

    let (_client, mut eventloop) = AsyncClient::new(opts, 16);

    // Drive the event loop in a background task and signal when ConnAck arrives.
    let (connected_tx, connected_rx) = tokio::sync::oneshot::channel::<()>();
    let el_handle = tokio::spawn(async move {
        let mut signal = Some(connected_tx);
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Packet::ConnAck(_))) => {
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

    eprintln!("[*] Connecting to {}:{}", args.connection.host, args.connection.port);

    tokio::time::timeout(Duration::from_secs(10), connected_rx)
        .await
        .context("timed out waiting for broker connection")?
        .ok();

    eprintln!("[+] Connected - will registered with broker");
    eprintln!("[*] Holding for {}s before triggering...", args.hold);

    sleep(Duration::from_secs(args.hold)).await;

    // Abort the event loop task - this drops the EventLoop, closing the TCP
    // socket without sending a DISCONNECT packet. The broker sees an ungraceful
    // disconnect and publishes the will message.
    el_handle.abort();
    eprintln!("[+] Disconnected ungracefully - will fired");

    if monitor_handle.is_some() {
        eprintln!("[*] Monitoring for {}s...", args.monitor_wait);
        sleep(Duration::from_secs(args.monitor_wait)).await;
    }

    if let Some(h) = monitor_handle {
        h.abort();
    }

    eprintln!("\n[+] Done");
    Ok(())
}

/// Subscribe to `topic` on a separate connection and print every received message.
async fn run_monitor(conn: ConnectionArgs, topic: String) {
    let opts = match build_options(&conn) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[!] Monitor setup failed: {}", e);
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
