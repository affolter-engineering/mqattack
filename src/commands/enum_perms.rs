use crate::commands::check_acl::{test_publish, test_subscribe, AclStatus};
use crate::mqtt::client::{parse_qos, ConnectionArgs};
use anyhow::{bail, Context, Result};
use clap::Args;
use std::fs::File;
use std::io::{BufRead, BufReader};

struct Credential {
    username: String,
    password: Option<String>,
}

struct UserResult {
    username: String,
    topics: Vec<TopicResult>,
}

struct TopicResult {
    topic: String,
    subscribe: AclStatus,
    publish: AclStatus,
}

#[derive(Args, Debug)]
pub struct EnumPermsArgs {
    #[command(flatten)]
    pub connection: ConnectionArgs,

    /// File with credentials, one "username:password" per line (# comments and blank lines ignored)
    #[arg(short = 'c', long)]
    pub credentials: String,

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

    /// QoS for publish probes (1 or 2 recommended for broker acknowledgement)
    #[arg(short = 'q', long, default_value_t = 1)]
    pub qos: u8,

    /// Seconds to wait for a broker response per test
    #[arg(long, default_value_t = 5)]
    pub wait: u64,
}

/// Run the enum-perms command
pub async fn run(args: EnumPermsArgs) -> Result<()> {
    if args.topic.is_none() && args.wordlist.is_none() {
        bail!("specify --topic <TOPIC> or --wordlist <FILE>");
    }
    if args.no_subscribe && args.no_publish {
        bail!("--no-subscribe and --no-publish cannot both be set");
    }

    let credentials = load_credentials(&args.credentials)?;
    if credentials.is_empty() {
        bail!("no credentials found in '{}'", args.credentials);
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
        "[*] {}:{} | {} user(s) | {} topic(s) | subscribe={} | publish={}\n",
        args.connection.host,
        args.connection.port,
        credentials.len(),
        topics.len(),
        check_sub,
        check_pub
    );

    let mut all_results: Vec<UserResult> = Vec::new();

    for cred in &credentials {
        eprintln!("[*] User: {}", cred.username);

        let mut conn = args.connection.clone();
        conn.username = Some(cred.username.clone());
        conn.password = cred.password.clone();

        let mut topic_results: Vec<TopicResult> = Vec::new();

        for topic in &topics {
            let sub_status = if check_sub {
                test_subscribe(&conn, topic, args.wait).await
            } else {
                AclStatus::Skipped
            };

            let pub_status = if check_pub {
                test_publish(&conn, topic, &payload, qos, args.wait).await
            } else {
                AclStatus::Skipped
            };

            eprintln!(
                "    {:<50}  sub={:<13}  pub={}",
                topic,
                sub_status.to_string(),
                pub_status
            );

            topic_results.push(TopicResult {
                topic: topic.clone(),
                subscribe: sub_status,
                publish: pub_status,
            });
        }

        eprintln!();
        all_results.push(UserResult {
            username: cred.username.clone(),
            topics: topic_results,
        });
    }

    print_matrix(&all_results, check_sub, check_pub);
    Ok(())
}


/// Print the permission matrix in a tabular format
fn print_matrix(results: &[UserResult], check_sub: bool, check_pub: bool) {
    if results.is_empty() || results[0].topics.is_empty() {
        return;
    }

    let topic_col = results[0]
        .topics
        .iter()
        .map(|r| r.topic.len())
        .max()
        .unwrap_or(5)
        .max(5);

    // Cell width: wide enough for the username and the cell value (+/+, -, etc.)
    let cell_width = results
        .iter()
        .map(|r| r.username.len())
        .max()
        .unwrap_or(4)
        .max(3);

    let legend = match (check_sub, check_pub) {
        (true, true) => "S=subscribe  P=publish  +=ALLOWED  -=DENIED  ?=INCONCLUSIVE",
        (true, false) => "S=subscribe  +=ALLOWED  -=DENIED  ?=INCONCLUSIVE",
        (false, true) => "P=publish  +=ALLOWED  -=DENIED  ?=INCONCLUSIVE",
        _ => return,
    };

    println!("\n[+] Permission Matrix");
    println!("    {}", legend);
    println!();

    // Header row
    print!("    {:<width$}", "Topic", width = topic_col + 2);
    for r in results {
        print!("  {:<width$}", r.username, width = cell_width);
    }
    println!();

    // Separator
    let sep_len = topic_col + 2 + results.len() * (cell_width + 2);
    println!("    {}", "-".repeat(sep_len));

    // Data rows
    for (i, tr) in results[0].topics.iter().enumerate() {
        print!("    {:<width$}", tr.topic, width = topic_col + 2);
        for user in results {
            let cell = format_cell(&user.topics[i], check_sub, check_pub);
            print!("  {:<width$}", cell, width = cell_width);
        }
        println!();
    }
}

/// Format a cell in the permission matrix based on the subscribe and publish status
fn format_cell(tr: &TopicResult, check_sub: bool, check_pub: bool) -> String {
    match (check_sub, check_pub) {
        (true, true) => format!("{}/{}", status_char(&tr.subscribe), status_char(&tr.publish)),
        (true, false) => format!("S:{}", status_char(&tr.subscribe)),
        (false, true) => format!("P:{}", status_char(&tr.publish)),
        _ => String::new(),
    }
}

/// Convert AclStatus to a single character for display
fn status_char(s: &AclStatus) -> char {
    match s {
        AclStatus::Allowed => '+',
        AclStatus::Denied => '-',
        AclStatus::Inconclusive => '?',
        AclStatus::Skipped => '.',
    }
}

/// Load credentials from a file
fn load_credentials(path: &str) -> Result<Vec<Credential>> {
    let file = File::open(path).with_context(|| format!("cannot open credentials file '{}'", path))?;
    let mut creds = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line.with_context(|| format!("error reading '{}'", path))?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // First ':' separates username from password; password may itself contain ':'.
        let (username, password) = match line.find(':') {
            Some(i) => {
                let pw = &line[i + 1..];
                (&line[..i], if pw.is_empty() { None } else { Some(pw.to_owned()) })
            }
            None => (line, None),
        };
        if username.is_empty() {
            continue;
        }
        creds.push(Credential {
            username: username.to_owned(),
            password,
        });
    }
    Ok(creds)
}

/// Load topics from a file or a single topic argument
fn load_topics(args: &EnumPermsArgs) -> Result<Vec<String>> {
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
