use crate::mqtt::client::{build_options, parse_qos, ConnectionArgs};
use anyhow::Result;
use clap::Args;
use rumqttc::{AsyncClient, Event, Packet};
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
    let opts = build_options(&args.connection)?;
    let (client, mut eventloop) = AsyncClient::new(opts, 64);

    // Enqueue subscriptions — they are sent once the connection is established.
    for topic in &args.topics {
        client.subscribe(topic, qos).await?;
    }

    eprintln!(
        "[*] Connecting to {}:{}",
        args.connection.host, args.connection.port
    );
    for topic in &args.topics {
        eprintln!("[*] Subscribing to '{}'", topic);
    }
    eprintln!("[*] Press Ctrl+C to stop\n");

    let mut received: u32 = 0;

    loop {
        tokio::select! {
            event = eventloop.poll() => {
                match event {
                    Ok(Event::Incoming(Packet::ConnAck(ack))) => {
                        eprintln!("[+] Connected (session_present={})", ack.session_present);
                    }
                    Ok(Event::Incoming(Packet::Publish(p))) => {
                        let ts = chrono::Local::now().format("%H:%M:%S%.3f");
                        let payload_str = if args.hex {
                            hex::encode(&p.payload)
                        } else {
                            String::from_utf8_lossy(&p.payload).into_owned()
                        };
                        println!(
                            "[{}] topic={} qos={} retain={} len={} | {}",
                            ts,
                            p.topic,
                            p.qos as u8,
                            p.retain,
                            p.payload.len(),
                            payload_str,
                        );
                        received += 1;
                        if let Some(max) = args.count {
                            if received >= max {
                                eprintln!("\n[*] Received {} message(s), stopping.", received);
                                client.disconnect().await.ok();
                                break;
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("[!] Connection error: {}", e);
                        break;
                    }
                }
            }
            _ = signal::ctrl_c() => {
                eprintln!("\n[*] Interrupted, disconnecting...");
                client.disconnect().await.ok();
                break;
            }
        }
    }

    Ok(())
}
