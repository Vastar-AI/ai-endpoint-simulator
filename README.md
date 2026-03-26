# AI Endpoint Simulator

A multi-dialect AI endpoint simulator built with Rust and actix-web. Simulates OpenAI, Anthropic, Ollama, Cohere, and Gemini streaming APIs with Redis-backed caching and configurable data sources (markdown files or ClickHouse database).

Built primarily for benchmarking and development testing of [VIL (Vastar Intermediate Language)](https://github.com/OceanOS-id/VIL) — a process-oriented zero-copy framework for distributed systems. The simulator provides all 5 SSE dialect formats that VIL's `SseSourceDialect` supports, enabling end-to-end pipeline testing without real AI provider accounts.

While designed for VIL, the simulator is fully compatible with any OpenAI/Anthropic/Ollama/Cohere/Gemini client — use it as a drop-in mock for any application that consumes these APIs.

## Features

- **5 SSE Dialects** — OpenAI, Anthropic, Ollama, Cohere, Gemini with correct response formats
- **Streaming Response** — Server-Sent Events with proper done-signal per dialect
- **Dual Data Source** — Markdown files or ClickHouse database
- **Redis Caching** — High-performance response caching with configurable TTL
- **Concurrency Control** — Semaphore-based rate limiting
- **Configurable Workers** — Multi-threaded actix-web with tunable worker count
- **Docker Support** — HAProxy load balancer with 3 replicas
- **~8K req/s** — Tested at 1000 concurrent connections, 100% success rate

## Supported Dialects

| Dialect | Endpoint | Done Signal | JSON Tap |
|---------|----------|-------------|----------|
| **OpenAI** | `POST /v1/chat/completions` | `data: [DONE]` | `choices[0].delta.content` |
| **Anthropic** | `POST /v1/messages` | `event: message_stop` | `delta.text` |
| **Ollama** | `POST /api/chat` | `"done": true` | `message.content` |
| **Cohere** | `POST /v1/chat` | `event: message-end` | `text` |
| **Gemini** | `POST /v1beta/models/:model:streamGenerateContent` | TCP EOF | `candidates[0].content.parts[0].text` |

## Quick Start

### Prerequisites

- Rust 1.70+ (stable)
- Redis 6+ (for caching)
- Docker (optional)

### Setup

```bash
git clone https://github.com/Vastar-AI/ai-endpoint-simulator.git
cd ai-endpoint-simulator
cp config.local.yml config.yml
cargo build --release
```

### Run

```bash
# Start Redis
docker run -d --name redis-simulator -p 6379:6379 redis:7-alpine

# Start simulator (with CPU pinning)
./run_simulator.sh

# Or manually
cargo run --release
```

### Docker Compose (with HAProxy + 3 replicas)

```bash
docker compose up -d
```

This starts Redis, 3 simulator replicas, and HAProxy load balancer on port 4545.

## Testing

```bash
# Health check
curl http://localhost:4545/health

# OpenAI
curl -N -X POST http://localhost:4545/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{"model":"gpt-4o","messages":[{"role":"user","content":"Hello"}],"stream":true}'

# Anthropic
curl -N -X POST http://localhost:4545/v1/messages \
  -H 'Content-Type: application/json' \
  -d '{"model":"claude-3-5-sonnet","messages":[{"role":"user","content":"Hello"}],"stream":true}'

# Ollama
curl -N -X POST http://localhost:4545/api/chat \
  -H 'Content-Type: application/json' \
  -d '{"model":"llama3","messages":[{"role":"user","content":"Hello"}],"stream":true}'

# Cohere
curl -N -X POST http://localhost:4545/v1/chat \
  -H 'Content-Type: application/json' \
  -d '{"messages":[{"role":"user","content":"Hello"}],"stream":true}'

# Gemini
curl -N -X POST 'http://localhost:4545/v1beta/models/gemini-1.5-pro:streamGenerateContent' \
  -H 'Content-Type: application/json' \
  -d '{"contents":[{"parts":[{"text":"Hello"}]}]}'

# Test connection (non-streaming)
curl -X POST http://localhost:4545/test_completion
```

## Performance

**System:** Intel i9-11900F (8C/16T), 32GB RAM, Ubuntu 22.04, Rust 1.93.1

| Dialect | req/s | P50 | P99 | Success |
|---------|-------|-----|-----|---------|
| OpenAI | 7,258 | 63ms | 103ms | 100% |
| Anthropic | 8,390 | 53ms | 87ms | 100% |
| Ollama | 6,689 | 70ms | 105ms | 100% |
| Cohere | 9,972 | 43ms | 71ms | 100% |
| Gemini | 7,271 | 65ms | 95ms | 100% |

Sweet spot: **c1000 — 8,262 req/s, P99 147ms**. Full results in [PERFORMANCE_REPORT.md](./PERFORMANCE_REPORT.md).

## Configuration

### config.yml

```yaml
binding:
  port: 4545
  host: 0.0.0.0
source: file                    # "file" or "database"
database:
  url: http://127.0.0.1:8123
  username: simulator_app
  password: "your_password"
redis:
  url: redis://127.0.0.1:6379
  prefix: ai_simulator
tracking:
  enabled: false
log_level: info
channel_capacity: 1000
semaphore_limit: 10000
workers: 8
cache_ttl: 60
```

### Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `source` | Data source: "file" or "database" | "file" |
| `binding.port` | Server listen port | 4545 |
| `workers` | Worker thread count | 8 |
| `semaphore_limit` | Max concurrent requests | 10000 |
| `channel_capacity` | SSE streaming channel capacity | 1000 |
| `cache_ttl` | Redis cache TTL in seconds | 60 |
| `redis.prefix` | Redis key prefix | "ai_simulator" |
| `log_level` | Log level: trace/debug/info/warn/error | "info" |
| `tracking.enabled` | Enable detailed request logging | false |

### Redis Cache Keys

| Key Pattern | Description | TTL |
|-------------|-------------|-----|
| `{prefix}:db_responses` | Cached database responses | `cache_ttl` |
| `{prefix}:file:{filename}` | Cached markdown file content | `cache_ttl` |
| `{prefix}:file_list` | Cached directory listing | 600s |

### ClickHouse Schema (if using database source)

```sql
CREATE TABLE response_simulator (
    qa_id UUID,
    pertanyaan String,
    jawaban String,
    referensi String
) ENGINE = MergeTree()
ORDER BY qa_id;
```

## Architecture

```
┌─────────────────────────────────────────────────┐
│                   HAProxy                        │
│                 (Port 4545)                      │
└──────────────────┬──────────────────────────────┘
                   │
     ┌─────────────┼─────────────┐
     │             │             │
     ▼             ▼             ▼
┌─────────┐ ┌─────────┐ ┌─────────┐
│ Replica 1│ │ Replica 2│ │ Replica 3│
└────┬────┘ └────┬────┘ └────┬────┘
     └────────────┼────────────┘
                  ▼
          ┌─────────────┐
          │    Redis     │
          │ (Shared Cache)│
          └─────────────┘
```

## Project Structure

```
ai-endpoint-simulator/
├── src/
│   ├── main.rs              # Entry point, HTTP handlers (5 dialects)
│   ├── stream.rs            # SSE streaming (OpenAI, Anthropic, Ollama, Cohere, Gemini)
│   ├── response.rs          # File and database response handling
│   └── config_loader.rs     # YAML configuration loading
├── zresponse/               # Markdown response files (if source=file)
├── config.yml               # Application config (Docker)
├── config.local.yml         # Application config (local dev)
├── docker-compose.yaml      # Docker Compose (HAProxy + 3 replicas)
├── haproxy.cfg              # HAProxy load balancer config
├── Dockerfile               # Multi-stage build (musl static linking)
├── run_simulator.sh         # Start script (Redis + CPU pinning)
├── stop_simulator.sh        # Stop script
├── benchmark.sh             # Load testing script
├── API_REFERENCE.md         # Full API documentation with examples
├── PERFORMANCE_REPORT.md    # Benchmark results and analysis
└── EXECUTIVE_SUMMARY.md     # Performance summary
```

## Deployment

### Production Config

```yaml
binding:
  port: 4545
  host: 0.0.0.0
source: file
redis:
  url: redis://your-redis-cluster:6379
  prefix: ai_prod
log_level: warn
channel_capacity: 1000
semaphore_limit: 5000
workers: 8
cache_ttl: 300
tracking:
  enabled: false
```

### Production Considerations

1. **Security** — Protect Redis and database credentials
2. **Redis** — Use persistence (AOF/RDB) and connection pooling
3. **Monitoring** — Track P99 latency, error rates, Redis cache hit ratio
4. **Scaling** — Adjust `workers` and `semaphore_limit` per server capacity
5. **Backup** — Regular backups of response files and database

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/new-dialect`)
3. Commit changes (`git commit -m 'Add new dialect support'`)
4. Push to branch (`git push origin feature/new-dialect`)
5. Open a Pull Request

## License

MIT License. See [LICENSE](./LICENSE) for details.

## Support

- [Issues](https://github.com/Vastar-AI/ai-endpoint-simulator/issues)
- [API Reference](./API_REFERENCE.md)

---

Part of the [VIL](https://github.com/OceanOS-id/VIL) ecosystem.
