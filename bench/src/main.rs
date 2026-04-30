use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const CABLE_SYSTEM: &str = "\
Every word earns its place. Paid per word — make them count.\n\
DROP: filler, articles, hedging, pleasantries, preamble, postamble, question restatement.\n\
KEEP: all technical content, warnings, exact errors, decisions, code blocks verbatim.\n\
FORMAT: bullets > prose. Fragments OK.";

// (prompt, signal_keywords, category)
const PROMPTS: &[(&str, &[&str], &str)] = &[
    // ── Technical CS explanations (10) ───────────────────────────────────────
    ("What is database connection pooling and why is it useful?",
     &["pool", "connection", "overhead", "reuse"], "cs-technical"),
    ("Explain how the Go garbage collector works.",
     &["GC", "mark", "sweep", "heap", "pause"], "cs-technical"),
    ("Why does Rust's borrow checker prevent data races?",
     &["ownership", "borrow", "lifetime", "race", "mut"], "cs-technical"),
    ("What is the difference between TCP and UDP?",
     &["TCP", "UDP", "reliable", "packet", "latency"], "cs-technical"),
    ("How does HTTPS work?",
     &["TLS", "certificate", "handshake", "encrypt", "key"], "cs-technical"),
    ("What is the CAP theorem?",
     &["consistency", "availability", "partition", "trade"], "cs-technical"),
    ("Explain async/await in Rust.",
     &["Future", "executor", "poll", "async", "await"], "cs-technical"),
    ("What are the tradeoffs of microservices vs monolith?",
     &["latency", "deploy", "scale", "complexity", "network"], "cs-technical"),
    ("How does a mutex prevent race conditions?",
     &["lock", "mutex", "thread", "race", "critical"], "cs-technical"),
    ("What is a bloom filter and when should I use one?",
     &["hash", "false", "memory", "set", "probabilistic"], "cs-technical"),

    // ── Short factual — expect low savings (8) ────────────────────────────────
    ("What year was the Linux kernel first released?",
     &["1991"], "short-factual"),
    ("What does TCP stand for?",
     &["Transmission", "Control", "Protocol"], "short-factual"),
    ("What is the default port for HTTPS?",
     &["443"], "short-factual"),
    ("Who created the Go programming language?",
     &["Google", "Pike", "Thompson"], "short-factual"),
    ("What is Git's default branch name?",
     &["main", "master"], "short-factual"),
    ("What does the HTTP 429 status code mean?",
     &["rate", "limit", "requests"], "short-factual"),
    ("What is the difference between == and === in JavaScript?",
     &["type", "coercion", "strict"], "short-factual"),
    ("What does SOLID stand for in software engineering?",
     &["Single", "Open", "Liskov", "Interface", "Dependency"], "short-factual"),

    // ── Code generation (8) ───────────────────────────────────────────────────
    ("Write a Python function that reverses a linked list.",
     &["def", "next", "node", "prev"], "code-generation"),
    ("Implement binary search in Go.",
     &["mid", "left", "right", "return"], "code-generation"),
    ("Write a SQL query to find the top 5 customers by total revenue.",
     &["SELECT", "ORDER", "LIMIT", "GROUP"], "code-generation"),
    ("Write a bash one-liner to find the 10 largest files in a directory.",
     &["du", "sort", "head"], "code-generation"),
    ("Implement a simple LRU cache in Python.",
     &["capacity", "evict", "OrderedDict"], "code-generation"),
    ("Write a Rust function that reads a file line by line and counts words.",
     &["BufReader", "lines", "split", "count"], "code-generation"),
    ("Write a regex to validate an email address.",
     &["@", "domain", "pattern"], "code-generation"),
    ("Implement a stack using two queues.",
     &["queue", "push", "pop", "enqueue"], "code-generation"),

    // ── Non-CS explanations (8) ───────────────────────────────────────────────
    ("Explain how photosynthesis works.",
     &["chlorophyll", "light", "CO2", "glucose", "oxygen"], "non-cs"),
    ("How does a car engine work?",
     &["combustion", "piston", "fuel", "cylinder"], "non-cs"),
    ("Explain the difference between a virus and a bacterium.",
     &["DNA", "cell", "replicate", "antibiotic"], "non-cs"),
    ("How does GPS work?",
     &["satellite", "signal", "triangulation", "time"], "non-cs"),
    ("Explain compound interest.",
     &["principal", "rate", "exponential", "interest"], "non-cs"),
    ("What causes inflation?",
     &["supply", "demand", "money", "price"], "non-cs"),
    ("How do vaccines work?",
     &["immune", "antibody", "antigen", "response"], "non-cs"),
    ("Explain how a neural network learns.",
     &["weight", "gradient", "loss", "backprop"], "non-cs"),

    // ── Architecture / debugging / practical (10) ─────────────────────────────
    ("How do you design a rate limiter?",
     &["bucket", "limit", "window", "request"], "architecture"),
    ("Explain event sourcing.",
     &["event", "state", "replay", "immutable"], "architecture"),
    ("When would you use a message queue?",
     &["queue", "async", "decouple", "consumer"], "architecture"),
    ("How do you handle database migrations safely in production?",
     &["migration", "rollback", "schema", "deploy"], "architecture"),
    ("My Docker container keeps restarting. How do I debug it?",
     &["logs", "exit", "inspect", "restart"], "architecture"),
    ("Why might my REST API be slow under high load?",
     &["bottleneck", "connection", "cache", "timeout"], "architecture"),
    ("What does SIGKILL vs SIGTERM mean and when should I use each?",
     &["kill", "signal", "graceful", "interrupt"], "architecture"),
    ("How do I safely rotate an API key in production without downtime?",
     &["rotate", "deploy", "secret", "rollback"], "architecture"),
    ("Explain the difference between concurrency and parallelism.",
     &["concurrent", "parallel", "thread", "time"], "architecture"),
    ("What is the difference between REST and GraphQL?",
     &["endpoint", "query", "schema", "overfetch"], "architecture"),

    // ── Creative / open-ended (6) ─────────────────────────────────────────────
    ("Write a haiku about debugging code.",
     &["line", "code"], "creative"),
    ("Give me 3 names for a startup that builds AI developer tools.",
     &["AI", "dev"], "creative"),
    ("Write a one-paragraph elevator pitch for a password manager app.",
     &["password", "secure", "store"], "creative"),
    ("Suggest 5 book recommendations for learning system design.",
     &["system", "design", "book"], "creative"),
    ("Write a tweet announcing a new open-source CLI tool.",
     &["open", "source", "CLI"], "creative"),
    ("Give me a 3-step morning routine for a software engineer.",
     &["morning", "focus", "energy"], "creative"),
];

#[derive(Parser)]
#[command(name = "cable-bench", about = "Measure token savings: baseline vs cable mode")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Ollama base URL (ollama backend)
    #[arg(long, default_value = "http://localhost:11434")]
    ollama: String,
    /// Model to use (overrides nodes.json gen_model; default: lfm2.5-thinking:latest)
    #[arg(long)]
    model: Option<String>,
    /// Output JSON file (default: cable_bench_{model}.json)
    #[arg(long, default_value = "")]
    output: String,
    /// Model emits <think> blocks — measure think vs response tokens separately
    #[arg(long)]
    thinking_model: bool,
    #[arg(long)]
    nodes_config: Option<PathBuf>,
    /// Backend to use
    #[arg(long, default_value = "ollama", value_enum)]
    backend: Backend,
    /// API base URL for openai backend (default: https://api.githubcopilot.com)
    #[arg(long, default_value = "https://api.githubcopilot.com")]
    api_base: String,
    /// API key for openai backend (falls back to GITHUB_TOKEN, then OPENAI_API_KEY env vars)
    #[arg(long)]
    api_key: Option<String>,
    /// Delay in milliseconds between each API request (default: 7000 for openai, 0 for ollama)
    #[arg(long)]
    request_delay_ms: Option<u64>,
}

#[derive(ValueEnum, Clone, PartialEq, Debug)]
enum Backend {
    Ollama,
    Openai,
}

#[derive(Subcommand)]
enum Command {
    /// Compare two result JSON files
    Compare { file1: String, file2: String },
}

#[derive(Deserialize, Clone, Debug)]
struct NodeEntry {
    name: String,
    base_url: String,
}

#[derive(Deserialize)]
struct NodesConfig {
    nodes: Vec<NodeEntry>,
    gen_model: Option<String>,
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    system: String,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: u32,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
    eval_count: Option<u64>,
}

// ── OpenAI / GitHub Copilot ──────────────────────────────────────────────────

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Serialize, Deserialize, Clone)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    usage: OpenAiUsage,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Deserialize)]
struct OpenAiUsage {
    completion_tokens: u64,
    #[serde(default)]
    completion_tokens_details: Option<OpenAiTokenDetails>,
}

#[derive(Deserialize)]
struct OpenAiTokenDetails {
    reasoning_tokens: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone)]
struct PromptResult {
    index: usize,
    category: String,
    node: String,
    baseline_tokens: u64,
    cable_tokens: u64,
    savings_pct: f64,
    signal: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    est_think_baseline: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    est_think_cable: Option<u64>,
}

#[derive(Serialize, Deserialize)]
struct RunResult {
    model: String,
    thinking_model: bool,
    prompts_run: usize,
    overall_savings_pct: f64,
    signal_preserved: usize,
    results: Vec<PromptResult>,
}

fn split_think(raw: &str) -> (usize, usize, &str) {
    if let (Some(s), Some(e)) = (raw.find("<think>"), raw.find("</think>")) {
        let think_chars = e.saturating_sub(s + 7);
        let after = raw[e + 8..].trim_start();
        (think_chars, after.len(), after)
    } else {
        (0, raw.len(), raw)
    }
}

fn chars_to_tokens(c: usize) -> u64 { (c as f64 / 4.0).round() as u64 }

fn run_prompt(client: &Client, ollama: &str, model: &str, prompt: &str, system: &str) -> Result<(u64, String)> {
    let resp: OllamaResponse = client
        .post(format!("{}/api/generate", ollama))
        .json(&OllamaRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            system: system.to_string(),
            stream: false,
            options: OllamaOptions { temperature: 0.1, num_predict: 2048 },
        })
        .send()?
        .json()?;
    Ok((resp.eval_count.unwrap_or(0), resp.response))
}

/// Returns (completion_tokens, response_text, reasoning_tokens_if_any)
fn run_prompt_openai(client: &Client, api_base: &str, api_key: &str, model: &str, prompt: &str, system: &str) -> Result<(u64, String, Option<u64>)> {
    let mut messages = vec![];
    if !system.is_empty() {
        messages.push(OpenAiMessage { role: "system".to_string(), content: system.to_string() });
    }
    messages.push(OpenAiMessage { role: "user".to_string(), content: prompt.to_string() });

    let req = OpenAiRequest { model: model.to_string(), messages, max_tokens: 2048, temperature: 0.1 };

    // Retry up to 5 times on 429
    for attempt in 0..5u32 {
        let http_resp = client
            .post(format!("{}/chat/completions", api_base))
            .bearer_auth(api_key)
            .json(&req)
            .send()?;

        let status = http_resp.status();

        if status.as_u16() == 429 {
            let body = http_resp.text().unwrap_or_default();
            // Parse "wait N seconds" from error message, default 60s
            let wait_secs: u64 = body.split_whitespace()
                .zip(body.split_whitespace().skip(1))
                .find_map(|(a, b)| if a == "wait" { b.parse().ok() } else { None })
                .unwrap_or(60)
                + 2; // small buffer
            eprintln!("  ⏳ Rate limited — waiting {}s (attempt {}/5)...", wait_secs, attempt + 1);
            thread::sleep(Duration::from_secs(wait_secs));
            continue;
        }

        let body = http_resp.text()?;
        if !status.is_success() {
            anyhow::bail!("API error {}: {}", status, body.chars().take(200).collect::<String>());
        }

        let resp: OpenAiResponse = serde_json::from_str(&body)
            .with_context(|| format!("Failed to parse response: {}", body.chars().take(300).collect::<String>()))?;

        let content = resp.choices.first().map(|c| c.message.content.clone()).unwrap_or_default();
        let reasoning = resp.usage.completion_tokens_details.and_then(|d| d.reasoning_tokens);
        return Ok((resp.usage.completion_tokens, content, reasoning));
    }

    anyhow::bail!("Rate limit retries exhausted")
}

fn check_signal(text: &str, keys: &[&str]) -> bool {
    let lower = text.to_lowercase();
    keys.iter().filter(|k| lower.contains(&k.to_lowercase())).count() >= keys.len() / 2
}

fn check_node_health(base_url: &str) -> bool {
    Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok()
        .and_then(|c| c.get(format!("{}/api/tags", base_url)).send().ok())
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

fn resolve_api_key(cli: &Cli) -> Option<String> {
    cli.api_key.clone()
        .or_else(|| std::env::var("GITHUB_TOKEN").ok())
        .or_else(|| std::env::var("OPENAI_API_KEY").ok())
}

fn run_bench(cli: &Cli) -> Result<String> {
    let is_openai = cli.backend == Backend::Openai;

    // Resolve API key for OpenAI backend
    let api_key = if is_openai {
        resolve_api_key(cli).context(
            "OpenAI backend requires an API key. Set --api-key, GITHUB_TOKEN, or OPENAI_API_KEY"
        )?
    } else {
        String::new()
    };

    // Priority: --model CLI > nodes.json gen_model > default
    let (nodes, model) = if is_openai {
        // OpenAI backend: single node, no nodes.json
        let m = cli.model.clone().unwrap_or_else(|| "gpt-4o".to_string());
        (vec![NodeEntry { name: "copilot".to_string(), base_url: cli.api_base.clone() }], m)
    } else if let Some(path) = &cli.nodes_config {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Cannot read nodes config: {}", path.display()))?;
        let cfg: NodesConfig = serde_json::from_str(&content).context("Invalid JSON in nodes config")?;
        let m = cli.model.clone()
            .or(cfg.gen_model)
            .unwrap_or_else(|| "lfm2.5-thinking:latest".to_string());
        (cfg.nodes, m)
    } else {
        let m = cli.model.clone().unwrap_or_else(|| "lfm2.5-thinking:latest".to_string());
        (vec![NodeEntry { name: "local".to_string(), base_url: cli.ollama.clone() }], m)
    };

    println!("\nCable Mode Benchmark");
    println!("Model: {}  Nodes: {}  Backend: {:?}", model, nodes.len(), cli.backend);
    if cli.thinking_model {
        println!("Thinking model: think tokens measured separately");
    }

    let healthy: Vec<NodeEntry> = if is_openai {
        // Cloud API — assume healthy, just print
        for n in &nodes { println!("  ✔  {} ({})", n.name, n.base_url); }
        nodes.clone()
    } else {
        nodes.iter()
            .filter(|n| {
                let ok = check_node_health(&n.base_url);
                println!("  {}  {} ({})", if ok { "✔" } else { "✗" }, n.name, n.base_url);
                ok
            })
            .cloned()
            .collect()
    };

    if healthy.is_empty() {
        anyhow::bail!("No healthy nodes found.");
    }
    println!("{}/{} nodes healthy\n", healthy.len(), nodes.len());

    let model = Arc::new(model);
    let healthy = Arc::new(healthy);
    let thinking_model = cli.thinking_model;
    let api_key = Arc::new(api_key);
    let api_base = Arc::new(cli.api_base.clone());
    let delay_ms = cli.request_delay_ms.unwrap_or(if is_openai { 7000 } else { 0 });

    let handles: Vec<_> = (0..healthy.len())
        .map(|node_idx| {
            let node = healthy[node_idx].clone();
            let model = Arc::clone(&model);
            let api_key = Arc::clone(&api_key);
            let api_base = Arc::clone(&api_base);
            let my_indices: Vec<usize> = (0..PROMPTS.len())
                .filter(|i| i % healthy.len() == node_idx)
                .collect();

            thread::spawn(move || -> Result<Vec<PromptResult>> {
                let client = Client::builder().timeout(Duration::from_secs(600)).build()?;
                let mut results = vec![];
                for idx in my_indices {
                    let (prompt, keys, category) = PROMPTS[idx];
                    println!("  [{}] #{} baseline...", node.name, idx + 1);

                    let use_openai = node.base_url.starts_with("https://") || node.base_url.contains("copilot") || node.base_url.contains("openai") || node.base_url.contains("azure");

                    let (bt, _br, think_b) = if use_openai {
                        if delay_ms > 0 { thread::sleep(Duration::from_millis(delay_ms)); }
                        let (t, r, reasoning) = run_prompt_openai(&client, &api_base, &api_key, &model, prompt, "")?;
                        let think = if thinking_model {
                            reasoning.or_else(|| { let (c,_,_) = split_think(&r); Some(chars_to_tokens(c)) })
                        } else { None };
                        (t, r, think)
                    } else {
                        let (t, r) = run_prompt(&client, &node.base_url, &model, prompt, "")?;
                        let think = if thinking_model { let (c,_,_) = split_think(&r); Some(chars_to_tokens(c)) } else { None };
                        (t, r, think)
                    };

                    println!("  [{}] #{} cable...", node.name, idx + 1);

                    let (tt, tr, think_t) = if use_openai {
                        if delay_ms > 0 { thread::sleep(Duration::from_millis(delay_ms)); }
                        let (t, r, reasoning) = run_prompt_openai(&client, &api_base, &api_key, &model, prompt, CABLE_SYSTEM)?;
                        let think = if thinking_model {
                            reasoning.or_else(|| { let (c,_,_) = split_think(&r); Some(chars_to_tokens(c)) })
                        } else { None };
                        (t, r, think)
                    } else {
                        let (t, r) = run_prompt(&client, &node.base_url, &model, prompt, CABLE_SYSTEM)?;
                        let think = if thinking_model { let (c,_,_) = split_think(&r); Some(chars_to_tokens(c)) } else { None };
                        (t, r, think)
                    };

                    let savings = if bt > 0 { bt.saturating_sub(tt) as f64 / bt as f64 * 100.0 } else { 0.0 };
                    let sig = check_signal(&tr, keys);
                    println!("  [{}] #{} done  ({}→{} tok, {:.1}%, {})",
                        node.name, idx + 1, bt, tt, savings, if sig { "✅" } else { "⚠️" });

                    results.push(PromptResult {
                        index: idx,
                        category: category.to_string(),
                        node: node.name.clone(),
                        baseline_tokens: bt,
                        cable_tokens: tt,
                        savings_pct: savings,
                        signal: sig,
                        est_think_baseline: think_b,
                        est_think_cable: think_t,
                    });
                }
                Ok(results)
            })
        })
        .collect();

    let mut all: Vec<PromptResult> = vec![];
    for handle in handles {
        match handle.join() {
            Ok(Ok(results)) => all.extend(results),
            Ok(Err(e)) => eprintln!("  ⚠ Node error: {}", e),
            Err(_) => eprintln!("  ⚠ Node thread panicked"),
        }
    }
    all.sort_by_key(|r| r.index);

    let total_b: u64 = all.iter().map(|r| r.baseline_tokens).sum();
    let total_t: u64 = all.iter().map(|r| r.cable_tokens).sum();
    let overall = if total_b > 0 { total_b.saturating_sub(total_t) as f64 / total_b as f64 * 100.0 } else { 0.0 };
    let signal_ok = all.iter().filter(|r| r.signal).count();

    // Print summary table
    println!("\n{}", "=".repeat(90));
    println!("{:<3} {:>10} {:>10} {:>8} {:>20} {:>12} {}",
        "#", "Baseline", "Telegram", "Savings", "Node", "Category", "Signal");
    println!("{}", "-".repeat(90));
    for r in &all {
        println!("  {:>2}  {:>10} {:>10} {:>7.1}%  {:>18}  {:>14}  {}",
            r.index + 1, r.baseline_tokens, r.cable_tokens, r.savings_pct,
            r.node, r.category, if r.signal { "✅" } else { "⚠️" });
    }
    println!("{}", "=".repeat(90));
    println!("  TOT  {:>10} {:>10} {:>7.1}%\n  Signal preserved: {}/{}",
        total_b, total_t, overall, signal_ok, all.len());

    let run = RunResult {
        model: (*model).clone(),
        thinking_model,
        prompts_run: all.len(),
        overall_savings_pct: overall,
        signal_preserved: signal_ok,
        results: all,
    };

    let safe_model = model.replace([':', '/', ' '], "_");
    let output_path = if cli.output.is_empty() {
        let suffix = if thinking_model { "_thinking" } else { "" };
        format!("cable_bench_{}{}.json", safe_model, suffix)
    } else {
        cli.output.clone()
    };

    std::fs::write(&output_path, serde_json::to_string_pretty(&run)?)?;
    println!("\n  Saved → {}\n", output_path);
    Ok(output_path)
}

fn cmd_compare(file1: &str, file2: &str) -> Result<()> {
    let load = |path: &str| -> Result<RunResult> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Cannot read: {}", path))?;
        serde_json::from_str(&content).with_context(|| format!("Invalid JSON: {}", path))
    };

    let a = load(file1)?;
    let b = load(file2)?;

    let cat_stats = |run: &RunResult| -> HashMap<String, (f64, usize, usize)> {
        let mut map: HashMap<String, (f64, usize, usize)> = HashMap::new();
        for r in &run.results {
            let e = map.entry(r.category.clone()).or_default();
            e.0 += r.savings_pct;
            e.1 += 1;
            if r.signal { e.2 += 1; }
        }
        map.into_iter().map(|(k, (sum, n, sig))| (k, (sum / n as f64, n, sig))).collect()
    };

    let a_cats = cat_stats(&a);
    let b_cats = cat_stats(&b);

    let w = 70;
    println!("\n{}", "=".repeat(w));
    println!("  Cable Mode Benchmark Comparison");
    println!("{}", "=".repeat(w));
    println!("  A: {}{}",
        a.model, if a.thinking_model { " (thinking)" } else { "" });
    println!("  B: {}{}",
        b.model, if b.thinking_model { " (thinking)" } else { "" });
    println!("{}", "-".repeat(w));
    println!("{:<28} {:>10} {:>10} {:>10}", "Metric", "A", "B", "Δ");
    println!("{}", "-".repeat(w));

    let delta = b.overall_savings_pct - a.overall_savings_pct;
    println!("{:<28} {:>9.1}% {:>9.1}% {:>+9.1}pp",
        "Overall savings", a.overall_savings_pct, b.overall_savings_pct, delta);
    println!("{:<28} {:>10} {:>10}",
        "Signal preserved",
        format!("{}/{}", a.signal_preserved, a.prompts_run),
        format!("{}/{}", b.signal_preserved, b.prompts_run));

    println!("\n  By category:");
    println!("{}", "-".repeat(w));
    let mut cats: Vec<String> = a_cats.keys().cloned().collect();
    cats.sort();
    for cat in &cats {
        if let (Some((a_avg, a_n, _)), Some((b_avg, _, _))) = (a_cats.get(cat), b_cats.get(cat)) {
            let d = b_avg - a_avg;
            println!("  {:<26} {:>9.1}% {:>9.1}% {:>+9.1}pp  (n={})",
                cat, a_avg, b_avg, d, a_n);
        }
    }

    if a.thinking_model || b.thinking_model {
        println!("\n  Think block estimates:");
        println!("{}", "-".repeat(w));
        for (label, run) in [("A", &a), ("B", &b)] {
            let tb: u64 = run.results.iter().filter_map(|r| r.est_think_baseline).sum();
            let tt: u64 = run.results.iter().filter_map(|r| r.est_think_cable).sum();
            if tb > 0 {
                let reduction = tb.saturating_sub(tt) as f64 / tb as f64 * 100.0;
                println!("  {} think baseline: {}  cable: {}  reduction: {:.1}%",
                    label, tb, tt, reduction);
            }
        }
    }

    println!("{}", "=".repeat(w));
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Some(Command::Compare { file1, file2 }) => cmd_compare(file1, file2),
        None => { run_bench(&cli)?; Ok(()) }
    }
}
