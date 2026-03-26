# AI Endpoint Simulator — Performance Report

Built for benchmarking [VIL (Vastar Intermediate Language)](https://github.com/OceanOS-id/VIL) SSE pipelines. Compatible with any AI API client.

**System:** Intel i9-11900F @ 2.50GHz (8C/16T), 32GB RAM, Ubuntu 22.04 (kernel 6.8.0)
**Rust:** 1.93.1 | **Framework:** actix-web 4 + Redis 7 cache | **Build:** `--release`
**Tool:** `oha` | **Date:** 2026-03-27
**Note:** Machine under normal development workload (load avg ~1.3) during all benchmarks.

---

## Per-Dialect Results (c500, n5000)

| Dialect | Endpoint | req/s | P50 | P99 | P99.9 | Success |
|---------|----------|-------|-----|-----|-------|---------|
| **OpenAI** | `/v1/chat/completions` | 7,258 | 63ms | 103ms | 113ms | 100% |
| **Anthropic** | `/v1/messages` | 8,390 | 53ms | 87ms | 124ms | 100% |
| **Ollama** | `/api/chat` | 6,689 | 70ms | 105ms | 121ms | 100% |
| **Cohere** | `/v1/chat` | 9,972 | 43ms | 71ms | 76ms | 100% |
| **Gemini** | `/v1beta/models/:m:stream` | 7,271 | 65ms | 95ms | 107ms | 100% |

All dialects: **100% success rate, zero errors.**

---

## Scaling (OpenAI dialect)

| Concurrent | Total Req | req/s | P50 | P99 | P99.9 | Slowest | Success |
|-----------|-----------|-------|-----|-----|-------|---------|---------|
| 500 | 5,000 | 7,258 | 63ms | 103ms | 113ms | 135ms | 100% |
| 600 | 6,000 | 7,120 | 77ms | 128ms | 142ms | 146ms | 100% |
| 700 | 7,000 | 7,039 | 91ms | 148ms | 164ms | 168ms | 100% |
| 800 | 8,000 | 7,926 | 92ms | 152ms | 166ms | 167ms | 100% |
| **1,000** | **10,000** | **8,262** | **115ms** | **147ms** | **162ms** | **174ms** | **100%** |
| 1,500 | 15,000 | 7,294 | 188ms | 275ms | 286ms | 300ms | 100% |
| 2,000 | 20,000 | 8,003 | 241ms | 294ms | 306ms | 313ms | 100% |

---

## Analysis

### Sweet Spot: c1000 — 8,262 req/s, P99 147ms

- At c1000, throughput peaks at ~8.2K req/s with P99 well within 200ms SLO.
- At c1500+, P50 exceeds 180ms — connection queuing becomes visible.
- At c2000, throughput stays ~8K but latency climbs past 250ms P50.

### Dialect Performance

- **Cohere** is fastest (~10K req/s) — smallest SSE payload, fewest events.
- **Anthropic** is second (~8.4K) — more SSE event types but efficient streaming.
- **OpenAI/Gemini** are similar (~7.2K) — standard SSE with moderate payload.
- **Ollama** slightly slower (~6.7K) — JSON field-based done detection adds marginal overhead.

### Bottleneck

The simulator is **NOT the bottleneck** when benchmarking VIL pipeline examples. At c500, all dialects deliver 7-10K req/s — far exceeding the ~3.5K req/s measured through VIL's SSE gateway pipeline (where upstream response time is the dominant factor).

---

## Reproduction

```bash
# Start simulator
./run_simulator.sh

# Per-dialect benchmark
oha -c 500 -n 5000 -m POST \
  -H 'Content-Type: application/json' \
  -d '{"model":"gpt-4o","messages":[{"role":"user","content":"Hi"}],"stream":true}' \
  http://localhost:4545/v1/chat/completions

# Scaling test
oha -c 1000 -n 10000 -m POST \
  -H 'Content-Type: application/json' \
  -d '{"model":"gpt-4o","messages":[{"role":"user","content":"Hi"}],"stream":true}' \
  http://localhost:4545/v1/chat/completions
```
