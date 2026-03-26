# Executive Summary — AI Endpoint Simulator

**Date:** 2026-03-27 | **Status:** Production Ready

---

## Bottom Line

**Multi-dialect SSE simulator achieving 7,000-10,000 req/s per dialect with 100% success rate** on a development machine under normal workload. Built for benchmarking [VIL (Vastar Intermediate Language)](https://github.com/OceanOS-id/VIL) pipelines — supports all 5 VIL `SseSourceDialect` variants: OpenAI, Anthropic, Ollama, Cohere, Gemini. Also compatible with any standard AI API client.

---

## Key Metrics

| Indicator | Value | Status |
|-----------|-------|--------|
| **Peak Throughput** | 9,972 req/s (Cohere) | Excellent |
| **Avg Throughput** | ~7,500 req/s (all dialects) | Excellent |
| **Reliability** | 100% success (all tests) | Perfect |
| **P50 Latency** | 43-70ms (at c500) | Excellent |
| **P99 Latency** | 71-105ms (at c500) | Excellent |
| **Supported Dialects** | 5 (OpenAI, Anthropic, Ollama, Cohere, Gemini) | Complete |

---

## Capacity

### At Optimal Load (c1000)
- **Per Second:** 8,262 requests
- **Per Hour:** 29.7 million requests
- **Per Day:** 713 million requests

### Projected (Dedicated Server, 2-3x)
- **Per Second:** 16,000-25,000 requests
- **Per Hour:** 57-90 million requests

---

## Dialect Comparison (c500, n5000)

```
Throughput by Dialect (req/s)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Cohere       ██████████████████████████  9,972
Anthropic    ██████████████████████      8,390
Gemini       ███████████████████         7,271
OpenAI       ██████████████████          7,258
Ollama       █████████████████           6,689
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

All dialects deliver correct SSE response formats with proper done-signal handling.

---

## Architecture

- **Runtime:** Rust + actix-web 4 (multi-threaded, async)
- **Caching:** Redis 7 with TTL-based eviction
- **Data Sources:** File-based (markdown) or ClickHouse database
- **Concurrency:** Semaphore-controlled with configurable limits
- **Deployment:** Docker Compose with HAProxy load balancing (3 replicas)

---

## Recommendations

1. **Use c500-c1000** for VIL pipeline benchmarking — well within SLO
2. **Deploy via Docker Compose** for multi-replica production use
3. **Set `AI_SIM_DELAY_MS=0`** for throughput testing, `20` for realistic latency
4. **Monitor P99 < 200ms** as the SLO boundary (exceeded at c1500+)

---

**Full Report:** See [PERFORMANCE_REPORT.md](./PERFORMANCE_REPORT.md)
**API Reference:** See [API_REFERENCE.md](./API_REFERENCE.md)
**Repository:** https://github.com/Vastar-AI/ai-endpoint-simulator
