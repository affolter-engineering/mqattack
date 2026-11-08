use crate::commands::check_acl::{test_publish, test_subscribe, AclStatus};
use crate::mqtt::client::{build_options, parse_qos, ConnectionArgs};
use anyhow::{bail, Context, Result};
use clap::Args;
use rumqttc::{AsyncClient, Event, Packet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use tokio::time::{timeout, Duration};

/// Arguments for the probe-anon command.
#[derive(Args, Debug)]
pub struct ProbeAnonArgs {
    #[command(flatten)]
    pub connection: ConnectionArgs,

    /// Single topic to probe ACLs on (mutually exclusive with --wordlist)
    #[arg(short = 't', long, conflicts_with = "wordlist")]
    pub topic: Option<String>,

    /// File with one topic per line to probe ACLs on (mutually exclusive with --topic)
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

/// Run the probe-anon command to test anonymous access and ACLs on specified topics.
pub async fn run(args: ProbeAnonArgs) -> Result<()> {
    if args.no_subscribe && args.no_publish {
        bail!("--no-subscribe and --no-publish cannot both be set");
    }

    let creds_provided = args.connection.username.is_some()
        || args.connection.password.is_some()
        || args.connection.jwt.is_some();
    if creds_provided {
        eprintln!("[!] Credentials ignored. probe-anon always tests unauthenticated access");
    }

    let anon_conn = ConnectionArgs {
        username: None,
        password: None,
        jwt: None,
        ..args.connection.clone()
    };

    eprintln!("[*] Target : {}:{}", anon_conn.host, anon_conn.port);

    if !check_anonymous_connect(&anon_conn, args.wait).await {
        println!("[-] Anonymous access DENIED. Broker requires authentication");
        return Ok(());
    }

    println!("[+] Anonymous access ALLOWED");

    if args.topic.is_none() && args.wordlist.is_none() {
        eprintln!("[*] No topics specified. Use --topic or --wordlist to probe ACLs.");
        return Ok(());
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
        "\n[*] {} topic(s) | subscribe={} | publish={}\n",
        topics.len(),
        check_sub,
        check_pub
    );

    let mut results: Vec<(String, AclStatus, AclStatus)> = Vec::new();

    for topic in &topics {
        let sub_status = if check_sub {
            test_subscribe(&anon_conn, topic, args.wait).await
        } else {
            AclStatus::Skipped
        };

        let pub_status = if check_pub {
            test_publish(&anon_conn, topic, &payload, qos, args.wait).await
        } else {
            AclStatus::Skipped
        };

        eprintln!(
            "    {:<50}  sub={:<13}  pub={}",
            topic,
            sub_status.to_string(),
            pub_status
        );

        results.push((topic.clone(), sub_status, pub_status));
    }

    let allowed_sub = results
        .iter()
        .filter(|(_, s, _)| *s == AclStatus::Allowed)
        .count();
    let denied_sub = results
        .iter()
        .filter(|(_, s, _)| *s == AclStatus::Denied)
        .count();
    let allowed_pub = results
        .iter()
        .filter(|(_, _, p)| *p == AclStatus::Allowed)
        .count();
    let denied_pub = results
        .iter()
        .filter(|(_, _, p)| *p == AclStatus::Denied)
        .count();

    println!();
    if check_sub {
        println!(
            "[+] Subscribe : {} allowed, {} denied, {} inconclusive",
            allowed_sub,
            denied_sub,
            results.len() - allowed_sub - denied_sub
        );
    }
    if check_pub {
        println!(
            "[+] Publish   : {} allowed, {} denied, {} inconclusive",
            allowed_pub,
            denied_pub,
            results.len() - allowed_pub - denied_pub
        );
    }

    let sub_allowed: Vec<_> = results
        .iter()
        .filter(|(_, s, _)| *s == AclStatus::Allowed)
        .collect();
    let pub_allowed: Vec<_> = results
        .iter()
        .filter(|(_, _, p)| *p == AclStatus::Allowed)
        .collect();

    if check_sub && !sub_allowed.is_empty() {
        println!("\n[+] Topics with subscribe ALLOWED:");
        for (t, _, _) in &sub_allowed {
            println!("    {}", t);
        }
    }
    if check_pub && !pub_allowed.is_empty() {
        println!("\n[+] Topics with publish ALLOWED:");
        for (t, _, _) in &pub_allowed {
            println!("    {}", t);
        }
    }

    Ok(())
}

/// Check if the broker allows anonymous connections within the specified wait time.
async fn check_anonymous_connect(conn: &ConnectionArgs, wait_secs: u64) -> bool {
    let opts = match build_options(conn) {
        Ok(o) => o,
        Err(_) => return false,
    };
    let (_client, mut eventloop) = AsyncClient::new(opts, 16);

    timeout(Duration::from_secs(wait_secs), async move {
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Packet::ConnAck(_))) => return true,
                Ok(_) => {}
                Err(_) => return false,
            }
        }
    })
    .await
    .unwrap_or(false)
}

/// Load topics from the command arguments or a wordlist file.
fn load_topics(args: &ProbeAnonArgs) -> Result<Vec<String>> {
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
