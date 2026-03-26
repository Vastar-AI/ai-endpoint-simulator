# AI Endpoint Simulator — API Reference

Built for benchmarking [VIL](https://github.com/OceanOS-id/VIL) pipelines. Compatible with any OpenAI/Anthropic/Ollama/Cohere/Gemini client.

**Base URL:** `http://localhost:4545` (default)
**Content-Type:** `application/json`
**Response Format:** JSON or Server-Sent Events (SSE)

---

## Endpoints

### `GET /health`

Health check endpoint.

```bash
curl http://localhost:4545/health
```

**Response:** `200 OK`
```json
{"service":"ai-endpoint-simulator","status":"healthy","timestamp":"2026-03-27T00:00:00Z"}
```

### `POST /test_completion`

Connection test endpoint (non-streaming).

```bash
curl -X POST http://localhost:4545/test_completion
```

**Response:** `200 OK`
```json
{
  "id": "chatcmpl-AjoahzpVUCsJmOQZRKZUze7qBjEjn",
  "object": "chat.completion",
  "model": "gpt-4o-2024-08-06",
  "choices": [{
    "index": 0,
    "message": { "role": "assistant", "content": "Connection successful!" },
    "finish_reason": "stop"
  }],
  "usage": { "prompt_tokens": 57, "completion_tokens": 92, "total_tokens": 149 }
}
```

---

## OpenAI Dialect

### `POST /v1/chat/completions`

OpenAI-compatible chat completion streaming (GPT-4, GPT-3.5, Mistral API).

**Request:**
```json
{
  "model": "gpt-4o",
  "messages": [{"role": "user", "content": "Hello"}],
  "stream": true
}
```

**SSE Response:**
```
data: {"id":"chatcmpl-xxx","object":"chat.completion.chunk","model":"gpt-4o-2024-08-06","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}

... (chunked tokens)

data: {"id":"chatcmpl-xxx","object":"chat.completion.chunk","choices":[],"usage":{"prompt_tokens":182,"completion_tokens":520,"total_tokens":702}}

data: [DONE]
```

**Done signal:** `data: [DONE]`
**VIL json_tap:** `choices[0].delta.content`

```bash
curl -N -X POST http://localhost:4545/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{"model":"gpt-4o","messages":[{"role":"user","content":"Hello"}],"stream":true}'
```

---

## Anthropic Dialect

### `POST /v1/messages`

Anthropic Claude Messages API streaming.

**Request:**
```json
{
  "model": "claude-3-5-sonnet",
  "messages": [{"role": "user", "content": "Hello"}],
  "max_tokens": 1024,
  "stream": true
}
```

**SSE Response:**
```
event: message_start
data: {"type":"message_start","message":{"id":"msg_xxx","model":"claude-3-5-sonnet","role":"assistant"}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

... (chunked tokens)

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":50}}

event: message_stop
data: {"type":"message_stop"}
```

**Done signal:** `event: message_stop`
**VIL json_tap:** `delta.text`

```bash
curl -N -X POST http://localhost:4545/v1/messages \
  -H 'Content-Type: application/json' \
  -d '{"model":"claude-3-5-sonnet","messages":[{"role":"user","content":"Hello"}],"stream":true}'
```

---

## Ollama Dialect

### `POST /api/chat`

Ollama local LLM chat streaming.

**Request:**
```json
{
  "model": "llama3",
  "messages": [{"role": "user", "content": "Hello"}],
  "stream": true
}
```

**SSE Response:**
```
data: {"model":"llama3","created_at":"2026-03-27T00:00:00Z","message":{"role":"assistant","content":"Hello"},"done":false}

... (chunked tokens)

data: {"model":"llama3","created_at":"2026-03-27T00:00:00Z","message":{"role":"assistant","content":""},"done":true,"total_duration":1500000000,"eval_count":50}
```

**Done signal:** `"done": true` (JSON field)
**VIL json_tap:** `message.content`

```bash
curl -N -X POST http://localhost:4545/api/chat \
  -H 'Content-Type: application/json' \
  -d '{"model":"llama3","messages":[{"role":"user","content":"Hello"}],"stream":true}'
```

---

## Cohere Dialect

### `POST /v1/chat`

Cohere Command R streaming.

**Request:**
```json
{
  "messages": [{"role": "user", "content": "Hello"}],
  "stream": true
}
```

**SSE Response:**
```
event: stream-start
data: {"is_finished":false,"event_type":"stream-start","generation_id":"gen-xxx"}

event: text-generation
data: {"is_finished":false,"event_type":"text-generation","text":"Hello"}

... (chunked tokens)

event: stream-end
data: {"is_finished":true,"event_type":"stream-end","finish_reason":"COMPLETE"}

event: message-end
data: [DONE]
```

**Done signal:** `event: message-end` + `data: [DONE]`
**VIL json_tap:** `text`

```bash
curl -N -X POST http://localhost:4545/v1/chat \
  -H 'Content-Type: application/json' \
  -d '{"messages":[{"role":"user","content":"Hello"}],"stream":true}'
```

---

## Gemini Dialect

### `POST /v1beta/models/{model}:streamGenerateContent`

Google Gemini streaming. The colon in the URL is a Gemini API convention.

**Request:**
```json
{
  "contents": [{"parts": [{"text": "Hello"}]}]
}
```

**SSE Response:**
```
data: {"candidates":[{"content":{"parts":[{"text":"Hello"}],"role":"model"},"finishReason":null,"index":0}],"modelVersion":"gemini-1.5-pro"}

... (chunked tokens)

data: {"candidates":[{"content":{"parts":[{"text":""}],"role":"model"},"finishReason":"STOP","index":0}],"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":50,"totalTokenCount":60}}
```

**Done signal:** TCP EOF (no explicit marker — connection closes after final chunk)
**VIL json_tap:** `candidates[0].content.parts[0].text`

```bash
curl -N -X POST 'http://localhost:4545/v1beta/models/gemini-1.5-pro:streamGenerateContent' \
  -H 'Content-Type: application/json' \
  -d '{"contents":[{"parts":[{"text":"Hello"}]}]}'
```

---

## VIL Dialect Mapping

| VIL `SseSourceDialect` | Simulator Endpoint | Done Detection |
|------------------------|--------------------|----------------|
| `SseSourceDialect::OpenAi` | `/v1/chat/completions` | `done_marker = "[DONE]"` |
| `SseSourceDialect::Anthropic` | `/v1/messages` | `done_event = "message_stop"` |
| `SseSourceDialect::Ollama` | `/api/chat` | `done_json_field = ("done", true)` |
| `SseSourceDialect::Cohere` | `/v1/chat` | `done_event = "message-end"` + `done_marker` |
| `SseSourceDialect::Gemini` | `/v1beta/models/:m:stream` | TCP EOF |

---

## Performance

| Dialect | req/s | P50 | P99 |
|---------|-------|-----|-----|
| OpenAI | 7,258 | 63ms | 103ms |
| Anthropic | 8,390 | 53ms | 87ms |
| Ollama | 6,689 | 70ms | 105ms |
| Cohere | 9,972 | 43ms | 71ms |
| Gemini | 7,271 | 65ms | 95ms |

Full results: [PERFORMANCE_REPORT.md](./PERFORMANCE_REPORT.md)

---

*[AI Endpoint Simulator](https://github.com/Vastar-AI/ai-endpoint-simulator) — Part of the [VIL](https://github.com/OceanOS-id/VIL) ecosystem*
