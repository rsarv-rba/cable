# cable

> *A cable was a telegram sent via submarine cable — same urgency, fewer words.*

**Cable** is a system prompt technique that compresses LLM outputs without losing signal. A single instruction — "every word earns its place, paid per word" — consistently cuts token usage by 50–60% on standard models while preserving technical accuracy.

---

## How it works

Cable adds one system prompt to every request:

```
Every word earns its place. Paid per word — make them count.
DROP: filler, articles, hedging, pleasantries, preamble, postamble, question restatement.
KEEP: all technical content, warnings, exact errors, decisions, code blocks verbatim.
FORMAT: bullets > prose. Fragments OK.
```

That's it. No fine-tuning, no post-processing.

---

## Results

Benchmarked across 50 prompts in 6 categories (cs-technical, code-generation, architecture, non-cs, short-factual, creative) using distributed Ollama nodes.

| Model | Backend | Token savings | Signal preserved |
|---|---|---|---|
| gemma4:e4b | Ollama | **55.6%** | 50/50 |
| claude-sonnet-4.6 | GitHub Copilot | **50.8%** | 49/50 |
| lfm2.5-thinking:latest | Ollama | 26.4% | 39/50 |

**Key finding:** Cable works well on standard models. On thinking models it backfires — the "be brief" instruction increases think-block token usage by ~17%, swamping output savings.

**Cloud models:** Claude Sonnet 4.6 via Copilot API — strongest on short-factual (61–91%) and code-generation (52–92%). Only 1 signal warning across 50 prompts.

---

## Files

```
cable/
├── cable.skill       # skill file (activates on /cable, "be brief", "less tokens")
└── bench/            # Rust benchmark tool
    ├── src/main.rs
    ├── Cargo.toml
    ├── nodes.json     # distributed node config
    └── run.sh         # interactive runner
```

---

## Benchmark

### Quick start

```bash
cd cable/bench
./run.sh
```

### Options

```
1) Build binary
2) Run benchmark (standard model)
3) Run benchmark (thinking model)
4) Run benchmark — distributed
5) Run benchmark — distributed (thinking model)
6) Run both — distributed, then compare
7) Compare two result files
```

### Distributed setup

Edit `nodes.json` with your Ollama node URLs:

```json
{
  "gen_model": "gemma4:e4b",
  "nodes": [
    { "name": "box1", "base_url": "http://localhost:11434" },
    { "name": "box2", "base_url": "http://10.0.0.22:11434" }
  ]
}
```

### OpenAI / GitHub Copilot backend

```bash
GITHUB_TOKEN=$(gh auth token) \
./target/release/cable-bench \
  --backend openai \
  --api-base https://api.githubcopilot.com \
  --model claude-sonnet-4.6
```

Flags:

| Flag | Default | Description |
|---|---|---|
| `--backend` | `ollama` | `ollama` or `openai` |
| `--api-base` | `https://api.githubcopilot.com` | API endpoint |
| `--api-key` | env: `GITHUB_TOKEN` → `OPENAI_API_KEY` | Auth token |
| `--request-delay-ms` | `7000` (openai), `0` (ollama) | Delay between requests |

Rate limits are handled automatically — 429 responses are retried with the wait time from the error body.

### Environment variables (Ollama)

| Var | Default | Description |
|---|---|---|
| `MODEL` | `gemma4:e4b` | Model for standard benchmark |
| `THINKING_MODEL` | `lfm2.5-thinking:latest` | Model for thinking benchmark |
| `NODES_CONFIG` | `nodes.json` | Path to node config |
| `OLLAMA_URL` | `http://localhost:11434` | Ollama URL (single-node mode) |

Override at runtime:
```bash
MODEL=llama3.2:3b ./run.sh 4
THINKING_MODEL=deepseek-r1:7b ./run.sh 6
```

### Output

Results saved as JSON: `cable_bench_{model}.json` / `cable_bench_{model}_thinking.json`

Compare two runs:
```bash
./run.sh compare cable_bench_gemma4_e4b.json cable_bench_deepseek-r1_7b_thinking.json
```

---

## Requirements

- [Rust](https://rustup.rs/) (stable)
- [Ollama](https://ollama.com/) running locally or on network nodes
