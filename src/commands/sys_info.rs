use crate::mqtt::client::{connect_and_subscribe, poll_loop, ConnectionArgs};
use anyhow::Result;
use clap::Args;
use rumqttc::QoS;
use std::collections::BTreeMap;
use tokio::signal;
use tokio::time::{timeout, Duration};

#[derive(Args, Debug)]
pub struct SysInfoArgs {
    #[command(flatten)]
    pub connection: ConnectionArgs,

    /// Seconds to wait for $SYS messages before printing results
    #[arg(short = 'w', long, default_value_t = 5)]
    pub wait: u64,

    /// Print values as they arrive instead of a summary at the end
    #[arg(long)]
    pub live: bool,
}

pub async fn run(args: SysInfoArgs) -> Result<()> {
    let mut conn = args.connection.clone();
    conn.client_id = format!("{}-sysinfo", conn.client_id);

    let topics = vec!["$SYS/#".to_string()];
    let (client, mut eventloop) =
        connect_and_subscribe(&conn, &topics, QoS::AtMostOnce).await?;

    eprintln!("[*] Connecting to {}:{}", conn.host, conn.port);
    eprintln!("[*] Subscribing to $SYS/#");
    if !args.live {
        eprintln!("[*] Collecting for {} second(s) ...\n", args.wait);
    }

    let wait_secs = args.wait;
    let live = args.live;
    let mut collected: BTreeMap<String, String> = BTreeMap::new();

    tokio::select! {
        _ = timeout(Duration::from_secs(wait_secs), poll_loop(&mut eventloop, |p| {
            let value = String::from_utf8_lossy(&p.payload).into_owned();
            if live {
                println!("{} = {}", p.topic, value);
            }
            collected.insert(p.topic.clone(), value);
            true // stop is time-based, always continue
        })) => {}
        _ = signal::ctrl_c() => {
            eprintln!("\n[*] Interrupted");
        }
    }

    client.disconnect().await.ok();

    if live {
        eprintln!("\n[*] {} topic(s) collected.", collected.len());
    } else {
        print_summary(&collected);
    }

    Ok(())
}

fn print_summary(map: &BTreeMap<String, String>) {
    if map.is_empty() {
        eprintln!("[!] No $SYS messages received. The broker may not publish $SYS or access is restricted.");
        return;
    }

    println!("{:<55} {}", "Topic", "Value");
    println!("{}", "-".repeat(80));

    let mut last_group = String::new();
    for (topic, value) in map {
        let group: String = topic.splitn(4, '/').take(3).collect::<Vec<_>>().join("/");
        if group != last_group {
            println!();
            last_group = group;
        }
        println!("{:<55} {}", topic, value);
    }

    println!("\n[*] {} topic(s) collected.", map.len());
}
