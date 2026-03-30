// src/stream.rs

use futures_util::Stream;
use tokio_stream::wrappers::ReceiverStream;
use tokio::sync::mpsc::{channel, Sender};
use log::{info, debug, error};
use rand::Rng;
use serde::Serialize;
use chrono;
use crate::CONFIG;

#[derive(Serialize)]
pub struct Chunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub system_fingerprint: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Serialize)]
pub struct Choice {
    pub index: u32,
    pub delta: Delta,
    pub logprobs: Option<serde_json::Value>,
    pub finish_reason: Option<String>,
}

#[derive(Serialize)]
pub struct Delta {
    pub content: String,
}

#[derive(Serialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub prompt_tokens_details: PromptTokensDetails,
    pub completion_tokens_details: CompletionTokensDetails,
}

#[derive(Serialize)]
pub struct PromptTokensDetails {
    pub cached_tokens: u32,
    pub audio_tokens: u32,
}

#[derive(Serialize)]
pub struct CompletionTokensDetails {
    pub reasoning_tokens: u32,
    pub audio_tokens: u32,
    pub accepted_prediction_tokens: u32,
    pub rejected_prediction_tokens: u32,
}

pub fn generate_id() -> String {
    let prefix = "chatcmpl-Ai";
    let suffix: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(30)
        .map(char::from)
        .collect();
    format!("{}{}", prefix, suffix)
}

fn split_into_chunks(input: &str) -> Vec<String> {
    let chunk_size = 10; // Adjust chunk size as needed
    input
        .as_bytes()
        .chunks(chunk_size)
        .map(|chunk| String::from_utf8_lossy(chunk).to_string())
        .collect()
}

async fn generate_chunks(tx: Sender<String>, input: &str) {
    info!("Generating chunks for input");
    let content_chunks = split_into_chunks(input);

    for content in content_chunks.iter() {
        let chunk = Chunk {
            id: generate_id(),
            object: "chat.completion.chunk".to_string(),
            created: 1735278816,
            model: "gpt-4o-2024-08-06".to_string(),
            system_fingerprint: "fp_d28bcae782".to_string(),
            choices: vec![Choice {
                index: 0,
                delta: Delta { content: content.clone() },
                logprobs: None,
                finish_reason: None,
            }],
            usage: None,
        };

        let chunk_str = serde_json::to_string(&chunk).unwrap();
        let combined_chunk = format!("data: {}\n\n", chunk_str);

        if let Err(_) = tx.send(combined_chunk.clone()).await {
            // Client disconnected — normal during benchmarks
            break;
        }
    }

    // Remove the final chunk sending from here
}

pub fn openai_simulator(input: &str) -> impl Stream<Item = String> {
    let (tx, rx) = channel(CONFIG.channel_capacity);
    let input = input.to_string();

    tokio::spawn(async move {
        generate_chunks(tx, &input).await;
    });

    ReceiverStream::new(rx)
}

// =============================================================================
// Multi-Dialect SSE Simulators
// =============================================================================
// Supports all VIL SseDialect variants: Anthropic, Ollama, Cohere, Gemini.
// OpenAI is the existing dialect above.

/// Anthropic Claude Messages API streaming.
/// done_event: event: message_stop
/// json_tap: delta.text
pub fn anthropic_simulator(input: &str, model: &str) -> impl Stream<Item = String> {
    let (tx, rx) = channel(CONFIG.channel_capacity);
    let input = input.to_string();
    let model = model.to_string();
    let id = generate_id().replace("chatcmpl-", "msg_");

    tokio::spawn(async move {
        // message_start
        let start = serde_json::json!({
            "type": "message_start",
            "message": { "id": id, "type": "message", "role": "assistant", "model": model, "content": [] }
        });
        let _ = tx.send(format!("event: message_start\ndata: {}\n\n", start)).await;

        // content_block_start
        let block_start = serde_json::json!({
            "type": "content_block_start", "index": 0,
            "content_block": { "type": "text", "text": "" }
        });
        let _ = tx.send(format!("event: content_block_start\ndata: {}\n\n", block_start)).await;

        // content_block_delta (tokens)
        let chunks = split_into_chunks(&input);
        for content in &chunks {
            let delta = serde_json::json!({
                "type": "content_block_delta", "index": 0,
                "delta": { "type": "text_delta", "text": content }
            });
            let _ = tx.send(format!("event: content_block_delta\ndata: {}\n\n", delta)).await;
        }

        // content_block_stop
        let _ = tx.send(format!("event: content_block_stop\ndata: {}\n\n",
            serde_json::json!({"type": "content_block_stop", "index": 0}))).await;

        // message_delta
        let msg_delta = serde_json::json!({
            "type": "message_delta",
            "delta": { "stop_reason": "end_turn" },
            "usage": { "output_tokens": chunks.len() }
        });
        let _ = tx.send(format!("event: message_delta\ndata: {}\n\n", msg_delta)).await;

        // message_stop (done signal)
        let _ = tx.send(format!("event: message_stop\ndata: {}\n\n",
            serde_json::json!({"type": "message_stop"}))).await;
    });

    ReceiverStream::new(rx)
}

/// Ollama chat streaming.
/// done_json_field: "done": true
/// json_tap: message.content
pub fn ollama_simulator(input: &str, model: &str) -> impl Stream<Item = String> {
    let (tx, rx) = channel(CONFIG.channel_capacity);
    let input = input.to_string();
    let model = model.to_string();

    tokio::spawn(async move {
        let chunks = split_into_chunks(&input);
        for content in &chunks {
            let chunk = serde_json::json!({
                "model": model,
                "created_at": chrono::Utc::now().to_rfc3339(),
                "message": { "role": "assistant", "content": content },
                "done": false
            });
            let _ = tx.send(format!("data: {}\n\n", chunk)).await;
        }

        // done signal
        let done = serde_json::json!({
            "model": model,
            "created_at": chrono::Utc::now().to_rfc3339(),
            "message": { "role": "assistant", "content": "" },
            "done": true,
            "total_duration": 1500000000_u64,
            "eval_count": chunks.len(),
            "eval_duration": 1200000000_u64
        });
        let _ = tx.send(format!("data: {}\n\n", done)).await;
    });

    ReceiverStream::new(rx)
}

/// Cohere Command R streaming.
/// done_event: event: message-end + data: [DONE]
/// json_tap: text
pub fn cohere_simulator(input: &str) -> impl Stream<Item = String> {
    let (tx, rx) = channel(CONFIG.channel_capacity);
    let input = input.to_string();
    let gen_id = generate_id().replace("chatcmpl-", "gen-");

    tokio::spawn(async move {
        // stream-start
        let start = serde_json::json!({
            "is_finished": false, "event_type": "stream-start", "generation_id": gen_id
        });
        let _ = tx.send(format!("event: stream-start\ndata: {}\n\n", start)).await;

        // text-generation
        let chunks = split_into_chunks(&input);
        for content in &chunks {
            let chunk = serde_json::json!({
                "is_finished": false, "event_type": "text-generation", "text": content
            });
            let _ = tx.send(format!("event: text-generation\ndata: {}\n\n", chunk)).await;
        }

        // stream-end
        let end = serde_json::json!({
            "is_finished": true, "event_type": "stream-end",
            "finish_reason": "COMPLETE", "response": { "generation_id": gen_id }
        });
        let _ = tx.send(format!("event: stream-end\ndata: {}\n\n", end)).await;

        // done signal
        let _ = tx.send("event: message-end\ndata: [DONE]\n\n".to_string()).await;
    });

    ReceiverStream::new(rx)
}

/// Google Gemini streaming.
/// done: TCP EOF (no explicit marker)
/// json_tap: candidates[0].content.parts[0].text
pub fn gemini_simulator(input: &str, model: &str) -> impl Stream<Item = String> {
    let (tx, rx) = channel(CONFIG.channel_capacity);
    let input = input.to_string();
    let model = model.to_string();

    tokio::spawn(async move {
        let chunks = split_into_chunks(&input);
        for content in &chunks {
            let chunk = serde_json::json!({
                "candidates": [{
                    "content": { "parts": [{ "text": content }], "role": "model" },
                    "finishReason": serde_json::Value::Null, "index": 0
                }],
                "modelVersion": model
            });
            let _ = tx.send(format!("data: {}\n\n", chunk)).await;
        }

        // final chunk with finishReason
        let final_chunk = serde_json::json!({
            "candidates": [{
                "content": { "parts": [{ "text": "" }], "role": "model" },
                "finishReason": "STOP", "index": 0
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": chunks.len(),
                "totalTokenCount": 10 + chunks.len()
            }
        });
        let _ = tx.send(format!("data: {}\n\n", final_chunk)).await;
        // Gemini: no explicit done signal — TCP EOF
    });

    ReceiverStream::new(rx)
}