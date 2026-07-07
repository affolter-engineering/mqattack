use crate::mqtt::client::{build_options, parse_qos, ConnectionArgs};
use anyhow::{bail, Context, Result};
use clap::Args;
use rumqttc::{AsyncClient, Event, Packet, QoS, SubscribeReasonCode};
use std::fs::File;
use std::io::{BufRead, BufReader};
use tokio::time::{timeout, Duration};

#[derive(Debug, Clone, PartialEq)]
pub enum AclStatus {
    Allowed,
    Denied,
    Inconclusive,
    Skipped,
}

impl std::fmt::Display for AclStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AclStatus::Allowed => write!(f, "ALLOWED"),
            AclStatus::Denied => write!(f, "DENIED"),
            AclStatus::Inconclusive => write!(f, "INCONCLUSIVE"),
            AclStatus::Skipped => write!(f, "SKIPPED"),
        }
    }
}

struct AclResult {
    topic: String,
    subscribe: AclStatus,
    publish: AclStatus,
}

#[derive(Args, Debug)]
pub struct CheckAclArgs {
    #[command(flatten)]
    pub connection: ConnectionArgs,

    /// Single topic to test (mutually exclusive with --wordlist)
    #[arg(short = 't', long, conflicts_with = "wordlist")]
    pub topic: Option<String>,

    /// File with one topic per line (mutually exclusive with --topic)
    #[arg(short = 'w', long, conflicts_with = "topic")]
    pub wordlist: Option<String>,

    /// Skip subscribe permission tests
    #[arg(long)]
    pub no_subscribe: bool,

    /// Skip publish permission tests
    #[arg(long)]
    pub no_publish: bool,

    /// Payload used for publish probes
    #[arg(long, default_value = "mqattack")]
    pub payload: String,

    /// QoS for publish probes (1 or 2 gives broker acknowledgement; 0 is always inconclusive)
    #[arg(short = 'q', long, default_value_t = 1)]
    pub qos: u8,

    /// Seconds to wait for a broker response per test
    #[arg(long, default_value_t = 5)]
    pub wait: u64,
}

pub async fn run(args: CheckAclArgs) -> Result<()> {
    if args.topic.is_none() && args.wordlist.is_none() {
        bail!("specify --topic <TOPIC> or --wordlist <FILE>");
    }
    if args.no_subscribe && args.no_publish {
        bail!("--no-subscribe and --no-publish cannot both be set");
    }

    let topics = load_topics(&args)?;
    if topics.is_empty() {
        bail!("no topics to test");
    }

    let qos = parse_qos(args.qos)?;
    let payload = args.payload.as_bytes().to_vec();
    let check_sub = !args.no_subscribe;
    let check_pub = !args.no_publish;

    eprintln!(
        "[*] Connecting to {}:{}",
        args.connection.host, args.connection.port
    );
    eprintln!(
        "[*] {} topic(s) | subscribe={} | publish={}\n",
        topics.len(),
        check_sub,
        check_pub
    );

    let mut results: Vec<AclResult> = Vec::new();

    for topic in &topics {
        let sub_status = if check_sub {
            test_subscribe(&args.connection, topic, args.wait).await
        } else {
            AclStatus::Skipped
        };

        let pub_status = if check_pub {
            test_publish(&args.connection, topic, &payload, qos, args.wait).await
        } else {
            AclStatus::Skipped
        };

        eprintln!(
            "    {:<50}  sub={:<13}  pub={}",
            topic, sub_status.to_string(), pub_status
        );

        results.push(AclResult {
            topic: topic.clone(),
            subscribe: sub_status,
            publish: pub_status,
        });
    }

    let allowed_sub = results.iter().filter(|r| r.subscribe == AclStatus::Allowed).count();
    let denied_sub  = results.iter().filter(|r| r.subscribe == AclStatus::Denied).count();
    let allowed_pub = results.iter().filter(|r| r.publish == AclStatus::Allowed).count();
    let denied_pub  = results.iter().filter(|r| r.publish == AclStatus::Denied).count();

    println!();
    if check_sub {
        println!(
            "[+] Subscribe : {} allowed, {} denied, {} inconclusive",
            allowed_sub, denied_sub, results.len() - allowed_sub - denied_sub
        );
    }
    if check_pub {
        println!(
            "[+] Publish   : {} allowed, {} denied, {} inconclusive",
            allowed_pub, denied_pub, results.len() - allowed_pub - denied_pub
        );
    }

    let sub_allowed: Vec<_> = results
        .iter()
        .filter(|r| r.subscribe == AclStatus::Allowed)
        .collect();
    let pub_allowed: Vec<_> = results
        .iter()
        .filter(|r| r.publish == AclStatus::Allowed)
        .collect();

    if check_sub && !sub_allowed.is_empty() {
        println!("\n[+] Topics with subscribe ALLOWED:");
        for r in &sub_allowed {
            println!("    {}", r.topic);
        }
    }
    if check_pub && !pub_allowed.is_empty() {
        println!("\n[+] Topics with publish ALLOWED:");
        for r in &pub_allowed {
            println!("    {}", r.topic);
        }
    }

    Ok(())
}

/// Connect, subscribe to `topic`, wait for a SubAck, and report the ACL outcome.
/// A broker that disconnects after the subscribe is sent is treated as DENIED.
pub async fn test_subscribe(conn: &ConnectionArgs, topic: &str, wait_secs: u64) -> AclStatus {
    let opts = match build_options(conn) {
        Ok(o) => o,
        Err(_) => return AclStatus::Inconclusive,
    };
    let (client, mut eventloop) = AsyncClient::new(opts, 16);

    // Queue subscribe; rumqttc sends it to the broker after ConnAck.
    if client.subscribe(topic, QoS::AtMostOnce).await.is_err() {
        return AclStatus::Inconclusive;
    }

    let status = timeout(Duration::from_secs(wait_secs), async move {
        let mut connected = false;
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Packet::ConnAck(_))) => {
                    connected = true;
                }
                Ok(Event::Incoming(Packet::SubAck(ack))) => {
                    let denied = ack
                        .return_codes
                        .iter()
                        .any(|c| matches!(c, SubscribeReasonCode::Failure));
                    return if denied { AclStatus::Denied } else { AclStatus::Allowed };
                }
                Ok(_) => {}
                Err(_) => {
                    // Broker disconnect after ConnAck but before SubAck → likely denied.
                    return if connected {
                        AclStatus::Denied
                    } else {
                        AclStatus::Inconclusive
                    };
                }
            }
        }
    })
    .await
    .unwrap_or(AclStatus::Inconclusive);

    client.disconnect().await.ok();
    status
}

/// Connect, publish to `topic` at `qos`, wait for an acknowledgement, and report the ACL outcome.
/// QoS 0 is always inconclusive because there is no broker acknowledgement.
/// A broker disconnect after the publish is treated as DENIED.
pub async fn test_publish(
    conn: &ConnectionArgs,
    topic: &str,
    payload: &[u8],
    qos: QoS,
    wait_secs: u64,
) -> AclStatus {
    let opts = match build_options(conn) {
        Ok(o) => o,
        Err(_) => return AclStatus::Inconclusive,
    };
    let (client, mut eventloop) = AsyncClient::new(opts, 16);
    let client_clone = client.clone();
    let topic_owned = topic.to_string();
    let payload_owned = payload.to_vec();

    let status = timeout(Duration::from_secs(wait_secs), async move {
        let mut published = false;
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Packet::ConnAck(_))) => {
                    if client_clone
                        .publish(&topic_owned, qos, false, payload_owned.clone())
                        .await
                        .is_err()
                    {
                        return AclStatus::Inconclusive;
                    }
                    published = true;
                    if qos == QoS::AtMostOnce {
                        // No acknowledgement possible at QoS 0.
                        return AclStatus::Inconclusive;
                    }
                }
                Ok(Event::Incoming(Packet::PubAck(_)))
                | Ok(Event::Incoming(Packet::PubComp(_))) => {
                    return AclStatus::Allowed;
                }
                Ok(_) => {}
                Err(_) => {
                    return if published {
                        AclStatus::Denied
                    } else {
                        AclStatus::Inconclusive
                    };
                }
            }
        }
    })
    .await
    .unwrap_or(AclStatus::Inconclusive);

    client.disconnect().await.ok();
    status
}

fn load_topics(args: &CheckAclArgs) -> Result<Vec<String>> {
    if let Some(t) = &args.topic {
        return Ok(vec![t.clone()]);
    }
    let path = args.wordlist.as_deref().unwrap();
    let file = File::open(path).with_context(|| format!("cannot open wordlist '{}'", path))?;
    let lines: std::io::Result<Vec<_>> = BufReader::new(file)
        .lines()
        .filter(|l| l.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(true))
        .collect();
    lines.with_context(|| format!("error reading wordlist '{}'", path))
}
