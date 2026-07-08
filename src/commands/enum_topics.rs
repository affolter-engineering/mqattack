use crate::mqtt::client::{build_options, parse_qos, poll_loop, ConnectionArgs};
use anyhow::{bail, Context, Result};
use clap::Args;
use rumqttc::AsyncClient;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use tokio::signal;
use tokio::time::{timeout, Duration};

/// Topics beyond this count require --force in brute-force mode.
const BRUTE_FORCE_CAP: usize = 100_000;

#[derive(Args, Debug)]
pub struct EnumTopicsArgs {
    #[command(flatten)]
    pub connection: ConnectionArgs,

    /// File containing one topic per line
    #[arg(short = 'w', long = "wordlist", value_name = "FILE", conflicts_with = "brute")]
    pub wordlist: Option<String>,

    /// Generate topic candidates by brute-force
    #[arg(long, conflicts_with = "wordlist")]
    pub brute: bool,

    /// Character set used in brute-force mode
    #[arg(long, default_value = "abcdefghijklmnopqrstuvwxyz0123456789_-")]
    pub charset: String,

    /// Maximum per-segment length in brute-force mode
    #[arg(long, default_value_t = 3)]
    pub max_length: usize,

    /// Number of /-separated levels to generate (1 = "word", 2 = "word/word", ...)
    #[arg(long, default_value_t = 1)]
    pub depth: usize,

    /// Prepend this prefix to every candidate (e.g. "home/")
    #[arg(long, default_value = "")]
    pub prefix: String,

    /// Seconds to collect messages per subscription batch
    #[arg(short = 't', long = "wait", default_value_t = 3)]
    pub wait: u64,

    /// Number of topics subscribed simultaneously per batch
    #[arg(long, default_value_t = 200)]
    pub batch_size: usize,

    /// QoS level for subscriptions (0, 1, or 2)
    #[arg(short = 'q', long, default_value_t = 0)]
    pub qos: u8,

    /// Bypass the brute-force candidate count safety cap
    #[arg(long)]
    pub force: bool,
}

/// Run the "enum-topics" command: enumerate active topics by subscribing to candidates.
pub async fn run(args: EnumTopicsArgs) -> Result<()> {
    if !args.brute && args.wordlist.is_none() {
        bail!("specify --wordlist <FILE> or --brute");
    }

    let candidates = build_candidates(&args)?;
    if candidates.is_empty() {
        bail!("no topic candidates to test");
    }

    let qos = parse_qos(args.qos)?;
    let opts = build_options(&args.connection)?;
    let (client, mut eventloop) = AsyncClient::new(opts, 512);

    let total = candidates.len();
    let batch_size = args.batch_size.max(1);
    let wait_secs = args.wait;
    let total_batches = (total + batch_size - 1) / batch_size;

    eprintln!("[*] Connecting to {}:{}", args.connection.host, args.connection.port);
    eprintln!(
        "[*] {} candidate(s) | {} batch(es) | {}s per batch",
        total, total_batches, wait_secs
    );
    eprintln!("[*] Press Ctrl+C to abort early\n");

    let mut found: Vec<String> = Vec::new();
    let mut aborted = false;

    for (batch_idx, batch) in candidates.chunks(batch_size).enumerate() {
        eprintln!(
            "[*] Batch {}/{} ({} topics)",
            batch_idx + 1,
            total_batches,
            batch.len()
        );

        for topic in batch {
            client.subscribe(topic.as_str(), qos).await?;
        }

        let batch_set: HashSet<String> = batch.iter().cloned().collect();
        let mut batch_hits: Vec<String> = Vec::new();

        tokio::select! {
            _ = timeout(Duration::from_secs(wait_secs), poll_loop(&mut eventloop, |p| {
                if batch_set.contains(&p.topic) && !batch_hits.contains(&p.topic) {
                    eprintln!("[+] FOUND: {}", p.topic);
                    batch_hits.push(p.topic.clone());
                }
                true
            })) => {}
            _ = signal::ctrl_c() => {
                eprintln!("\n[*] Aborted.");
                aborted = true;
            }
        }

        found.extend(batch_hits);

        for topic in batch {
            client.unsubscribe(topic.as_str()).await.ok();
        }

        if aborted {
            break;
        }
    }

    client.disconnect().await.ok();

    println!();
    if found.is_empty() {
        eprintln!(
            "[*] No active topics found{}.",
            if aborted { " (scan incomplete)" } else { "" }
        );
    } else {
        let mut sorted = found.clone();
        sorted.sort();
        println!("[+] Found {} active topic(s):", sorted.len());
        for t in &sorted {
            println!("    {}", t);
        }
    }

    Ok(())
}

/// Build the list of candidate topics from either a wordlist or brute-force generation.
fn build_candidates(args: &EnumTopicsArgs) -> Result<Vec<String>> {
    let raw: Vec<String> = if args.brute {
        let charset_len = args.charset.chars().count();
        if charset_len == 0 {
            bail!("--charset must not be empty");
        }
        if args.max_length == 0 {
            bail!("--max-length must be at least 1");
        }
        if args.depth == 0 {
            bail!("--depth must be at least 1");
        }

        // Estimate before allocating to catch explosions early.
        let seg_count = estimate_count(charset_len, args.max_length);
        let total = seg_count.saturating_pow(args.depth as u32);
        if total > BRUTE_FORCE_CAP as u64 && !args.force {
            bail!(
                "brute-force would generate ~{} candidates (safety cap: {}). \
                 Reduce --max-length or --depth, or pass --force to override.",
                total,
                BRUTE_FORCE_CAP
            );
        }
        if total > BRUTE_FORCE_CAP as u64 {
            eprintln!("[!] WARNING: generating ~{} candidates — this may take a while.", total);
        }

        let segments = gen_segments(&args.charset, args.max_length);
        combine_depth(&segments, args.depth)
    } else {
        let path = args.wordlist.as_deref().unwrap();
        load_wordlist(path)?
    };

    let prefix = args.prefix.trim_end_matches('/');
    Ok(raw
        .into_iter()
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
        .map(|t| {
            if prefix.is_empty() {
                t
            } else {
                format!("{}/{}", prefix, t)
            }
        })
        .collect())
}

/// Load a wordlist file into a vector of strings, one per line.
fn load_wordlist(path: &str) -> Result<Vec<String>> {
    let file = File::open(path).with_context(|| format!("cannot open wordlist '{}'", path))?;
    BufReader::new(file)
        .lines()
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("error reading wordlist '{}'", path))
}

/// Estimate total segment strings: sum(charset_len^k, k = 1..=max_len)
fn estimate_count(charset_len: usize, max_len: usize) -> u64 {
    let mut total: u64 = 0;
    let mut power: u64 = 1;
    for _ in 1..=max_len {
        power = power.saturating_mul(charset_len as u64);
        total = total.saturating_add(power);
    }
    total
}

/// Generate all strings of length 1..=max_len over the given charset.
fn gen_segments(charset: &str, max_len: usize) -> Vec<String> {
    let chars: Vec<char> = charset.chars().collect();
    let mut out = Vec::new();
    for len in 1..=max_len {
        gen_rec(&chars, len, String::new(), &mut out);
    }
    out
}

fn gen_rec(chars: &[char], remaining: usize, current: String, result: &mut Vec<String>) {
    if remaining == 0 {
        result.push(current);
        return;
    }
    for &c in chars {
        let mut next = current.clone();
        next.push(c);
        gen_rec(chars, remaining - 1, next, result);
    }
}

/// Combine single-level segments into `depth`-level paths separated by '/'.
fn combine_depth(segments: &[String], depth: usize) -> Vec<String> {
    if depth <= 1 {
        return segments.to_vec();
    }
    let sub = combine_depth(segments, depth - 1);
    let mut result = Vec::with_capacity(segments.len() * sub.len());
    for seg in segments {
        for s in &sub {
            result.push(format!("{}/{}", seg, s));
        }
    }
    result
}

/// Unit tests for topic enumeration
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_count_single_char_max1() {
        assert_eq!(estimate_count(1, 1), 1);
    }

    #[test]
    fn estimate_count_binary_alphabet_depth3() {
        // 2^1 + 2^2 + 2^3 = 2 + 4 + 8 = 14
        assert_eq!(estimate_count(2, 3), 14);
    }

    #[test]
    fn gen_segments_length1() {
        let segs = gen_segments("ab", 1);
        assert_eq!(segs, vec!["a", "b"]);
    }

    #[test]
    fn gen_segments_length2_contains_all_pairs() {
        let segs = gen_segments("ab", 2);
        // Length-1 first, then length-2
        assert!(segs.contains(&"a".to_string()));
        assert!(segs.contains(&"b".to_string()));
        assert!(segs.contains(&"aa".to_string()));
        assert!(segs.contains(&"ab".to_string()));
        assert!(segs.contains(&"ba".to_string()));
        assert!(segs.contains(&"bb".to_string()));
        assert_eq!(segs.len(), 6); // 2 + 4
    }

    #[test]
    fn combine_depth_1_is_identity() {
        let segs: Vec<String> = vec!["a".into(), "b".into()];
        assert_eq!(combine_depth(&segs, 1), segs);
    }

    #[test]
    fn combine_depth_2_joins_with_slash() {
        let segs: Vec<String> = vec!["x".into(), "y".into()];
        let mut result = combine_depth(&segs, 2);
        result.sort();
        assert_eq!(result, vec!["x/x", "x/y", "y/x", "y/y"]);
    }

    #[test]
    fn combine_depth_3_produces_three_levels() {
        let segs: Vec<String> = vec!["a".into()];
        let result = combine_depth(&segs, 3);
        assert_eq!(result, vec!["a/a/a"]);
    }
}
