use crate::mqtt::client::{connect_and_subscribe, parse_qos, poll_loop, ConnectionArgs};
use anyhow::Result;
use clap::Args;
use tokio::signal;

#[derive(Args, Debug)]
pub struct SubscribeArgs {
    #[command(flatten)]
    pub connection: ConnectionArgs,

    /// Topic filter(s) to subscribe to (repeatable: -t t1 -t t2)
    #[arg(short = 't', long = "topic", required = true)]
    pub topics: Vec<String>,

    /// QoS level for the subscription (0, 1, or 2)
    #[arg(short = 'q', long, default_value_t = 0)]
    pub qos: u8,

    /// Exit after receiving N messages
    #[arg(short = 'C', long)]
    pub count: Option<u32>,

    /// Print payload bytes as hexadecimal instead of UTF-8
    #[arg(long)]
    pub hex: bool,
}

pub async fn run(args: SubscribeArgs) -> Result<()> {
    let qos = parse_qos(args.qos)?;
    let (client, mut eventloop) =
        connect_and_subscribe(&args.connection, &args.topics, qos).await?;

    eprintln!(
        "[*] Connecting to {}:{}",
        args.connection.host, args.connection.port
    );
    for topic in &args.topics {
        eprintln!("[*] Subscribing to '{}'", topic);
    }
    eprintln!("[*] Press Ctrl+C to stop\n");

    let count = args.count;
    let hex = args.hex;
    let mut received: u32 = 0;

    tokio::select! {
        _ = poll_loop(&mut eventloop, |p| {
            let ts = chrono::Local::now().format("%H:%M:%S%.3f");
            let payload_str = if hex {
                hex::encode(&p.payload)
            } else {
                String::from_utf8_lossy(&p.payload).into_owned()
            };
            println!(
                "[{}] topic={} qos={} retain={} len={} | {}",
                ts, p.topic, p.qos as u8, p.retain, p.payload.len(), payload_str,
            );
            received += 1;
            count.map_or(true, |max| received < max)
        }) => {
            if let Some(max) = count {
                if received >= max {
                    eprintln!("\n[*] Received {} message(s), stopping.", received);
                }
            }
        }
        _ = signal::ctrl_c() => {
            eprintln!("\n[*] Interrupted, disconnecting ...");
        }
    }

    client.disconnect().await.ok();
    Ok(())
}
