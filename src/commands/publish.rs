use crate::mqtt::client::{build_options, parse_qos, ConnectionArgs};
use anyhow::Result;
use clap::Args;
use rumqttc::{AsyncClient, Event, Outgoing, Packet, QoS};
use std::io::Read;

#[derive(Args, Debug)]
pub struct PublishArgs {
    #[command(flatten)]
    pub connection: ConnectionArgs,

    /// Topic to publish to
    #[arg(short = 't', long)]
    pub topic: String,

    /// Message payload. Use '-' to read from stdin
    #[arg(short = 'm', long)]
    pub message: String,

    /// QoS level (0, 1, or 2)
    #[arg(short = 'q', long, default_value_t = 0)]
    pub qos: u8,

    /// Set the retain flag on the published message
    #[arg(short = 'r', long)]
    pub retain: bool,
}

pub async fn run(args: PublishArgs) -> Result<()> {
    let qos = parse_qos(args.qos)?;
    let opts = build_options(&args.connection)?;
    let (client, mut eventloop) = AsyncClient::new(opts, 16);

    let payload: Vec<u8> = if args.message == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf.into_bytes()
    } else {
        args.message.into_bytes()
    };

    // Enqueue the publish before the first poll; it is sent once connected.
    client
        .publish(&args.topic, qos, args.retain, payload)
        .await?;

    eprintln!(
        "[*] Connecting to {}:{}",
        args.connection.host, args.connection.port
    );
    eprintln!("[*] Publishing to '{}'", args.topic);

    let mut done = false;

    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::ConnAck(_))) => {
                eprintln!("[+] Connected");
            }
            // QoS 0 — no broker acknowledgement; consider done after the packet leaves.
            Ok(Event::Outgoing(Outgoing::Publish(_))) => {
                if qos == QoS::AtMostOnce && !done {
                    done = true;
                    eprintln!("[+] Message sent (QoS 0, no ack)");
                    client.disconnect().await.ok();
                }
            }
            // QoS 1 — broker acknowledged with PUBACK.
            Ok(Event::Incoming(Packet::PubAck(_))) => {
                if !done {
                    done = true;
                    eprintln!("[+] Message acknowledged (QoS 1)");
                    client.disconnect().await.ok();
                }
            }
            // QoS 2 — four-way handshake complete with PUBCOMP.
            Ok(Event::Incoming(Packet::PubComp(_))) => {
                if !done {
                    done = true;
                    eprintln!("[+] Message acknowledged (QoS 2)");
                    client.disconnect().await.ok();
                }
            }
            // Disconnect packet sent; we are clean to exit.
            Ok(Event::Outgoing(Outgoing::Disconnect)) => {
                if done {
                    break;
                }
            }
            Ok(_) => {}
            Err(e) => {
                if done {
                    // An error after a clean disconnect is expected.
                    break;
                }
                return Err(anyhow::anyhow!("connection error: {}", e));
            }
        }
    }

    Ok(())
}
