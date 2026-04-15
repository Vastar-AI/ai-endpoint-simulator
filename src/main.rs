mod stream;
mod response;
mod config_loader;
pub mod embedded_responses;

use std::sync::Arc;
use std::time::Instant;
use actix_web::{web, App, HttpResponse, HttpServer, middleware::Logger, ResponseError};
use tokio::sync::Semaphore;
use futures_util::StreamExt;
use log::{info, debug, error};
use derive_more::Display;
use serde::Deserialize;
use dashmap::DashMap;
use crate::response::read_random_markdown_file_async;
use chrono;
use crate::stream::{openai_simulator, anthropic_simulator, ollama_simulator, cohere_simulator, gemini_simulator, Chunk, generate_id, PromptTokensDetails, Usage, CompletionTokensDetails};
use crate::config_loader::Config;
use env_logger::Builder;
use once_cell::sync::Lazy;

#[derive(Debug, Display)]
enum CustomError {
    #[display(fmt = "Failed to fetch responses")]
    FetchError,
    #[display(fmt = "Failed to bind server: {}", _0)]
    BindError(String),
}

impl ResponseError for CustomError {}

impl From<std::io::Error> for CustomError {
    fn from(error: std::io::Error) -> Self {
        CustomError::BindError(error.to_string())
    }
}

static CONFIG: Lazy<Config> = Lazy::new(|| Config::load());

/// In-memory cache entry with TTL
struct CacheEntry {
    value: String,
    expires_at: Instant,
}

/// Application state — in-memory cache, no external dependencies
struct AppState {
    cache: DashMap<String, CacheEntry>,
    file_list: DashMap<String, (Vec<String>, Instant)>,
}

impl AppState {
    fn new() -> Self {
        Self {
            cache: DashMap::new(),
            file_list: DashMap::new(),
        }
    }

    fn get_cached(&self, key: &str) -> Option<String> {
        if let Some(entry) = self.cache.get(key) {
            if entry.expires_at > Instant::now() {
                return Some(entry.value.clone());
            }
            drop(entry);
            self.cache.remove(key);
        }
        None
    }

    fn set_cached(&self, key: String, value: String, ttl_secs: u64) {
        self.cache.insert(key, CacheEntry {
            value,
            expires_at: Instant::now() + std::time::Duration::from_secs(ttl_secs),
        });
    }
}

/// Get cached file content, or read from disk/embedded if cache miss
async fn get_cached_file_response(state: &AppState, folder_path: &str) -> Result<String, CustomError> {
    // Get or scan file list
    let files: Vec<String> = {
        let needs_scan = match state.file_list.get(folder_path) {
            Some(entry) if entry.1 > Instant::now() => false,
            _ => true,
        };

        if needs_scan {
            let folder = folder_path.to_string();
            let scanned = tokio::task::spawn_blocking(move || {
                std::fs::read_dir(&folder)
                    .map(|entries| {
                        entries
                            .filter_map(Result::ok)
                            .filter(|entry| entry.path().extension().map_or(false, |ext| ext == "md"))
                            .filter_map(|entry| entry.file_name().into_string().ok())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_else(|_| Vec::new())
            })
            .await
            .map_err(|_| CustomError::FetchError)?;

            if !scanned.is_empty() {
                state.file_list.insert(
                    folder_path.to_string(),
                    (scanned.clone(), Instant::now() + std::time::Duration::from_secs(600)),
                );
            }
            scanned
        } else {
            state.file_list.get(folder_path).unwrap().0.clone()
        }
    };

    // If no files on disk, use embedded responses directly
    if files.is_empty() {
        info!("Using embedded response data (no {} folder)", folder_path);
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        let content = embedded_responses::EMBEDDED_RESPONSES
            .choose(&mut rng)
            .expect("No embedded responses");
        return Ok(content.to_string());
    }

    // Select random file and check cache
    let random_idx = rand::random::<usize>() % files.len();
    let selected_file = &files[random_idx];
    let cache_key = format!("file:{}", selected_file);

    if let Some(cached) = state.get_cached(&cache_key) {
        debug!("Cache hit: {}", selected_file);
        return Ok(cached);
    }

    // Cache miss — read from disk
    info!("Cache miss, reading file from disk: {}/{}", folder_path, selected_file);
    let content = read_random_markdown_file_async(folder_path).await.map_err(|e| {
        error!("Failed to read markdown file: {}", e);
        CustomError::FetchError
    })?;

    state.set_cached(cache_key, content.clone(), CONFIG.cache_ttl);
    Ok(content)
}

#[actix_web::get("/health")]
async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "ai-endpoint-simulator",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

#[actix_web::post("/test_completion")]
async fn test_completion() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "id": "chatcmpl-AjoahzpVUCsJmOQZRKZUze7qBjEjn",
        "object": "chat.completion",
        "created": 1735482595,
        "model": "gpt-4o-2024-08-06",
        "choices": [
            {
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "============>>  Selamat! Aplikasi anda telah sukses terhubung ke OpenAI Simulator. <============="
                },
                "logprobs": null,
                "finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": 57,
            "completion_tokens": 92,
            "total_tokens": 149
        }
    }))
}

/// OpenAI-compatible request body (partial — fields we care about)
#[derive(Debug, Deserialize, Default)]
struct OpenAiRequest {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    messages: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    stream: Option<bool>,
    #[serde(default)]
    tools: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    #[allow(dead_code)]
    max_tokens: Option<u64>,
}

#[actix_web::post("/v1/chat/completions")]
async fn chat_completions(
    state: web::Data<Arc<AppState>>,
    semaphore: web::Data<Arc<Semaphore>>,
    body: web::Json<OpenAiRequest>,
) -> Result<HttpResponse, CustomError> {
    let _permit = semaphore.acquire().await.map_err(|_| CustomError::FetchError)?;

    let model = body.model.clone().unwrap_or_else(|| "gpt-4o-2024-08-06".into());
    let wants_stream = body.stream.unwrap_or(true);
    let has_tools = body.tools.as_ref().map_or(false, |t| !t.is_empty());

    info!(
        "Chat completions: model={}, stream={}, tools={}",
        model, wants_stream, has_tools
    );

    // ── Tool calls mode ──
    if has_tools {
        // Check if messages already contain tool results → agent is in follow-up turn
        // In that case, return a text answer to end the ReAct loop
        let has_tool_results = body.messages.as_ref().map_or(false, |msgs| {
            msgs.iter().any(|m| m["role"] == "tool")
        });

        if has_tool_results {
            // Agent already executed tools — return final text answer
            let random_response = get_cached_file_response(&state, "zresponse").await?;
            let content: String = random_response.chars().take(1000).collect();
            let resp = serde_json::json!({
                "id": generate_id(),
                "object": "chat.completion",
                "created": chrono::Utc::now().timestamp(),
                "model": &model,
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": content
                    },
                    "logprobs": null,
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 200,
                    "completion_tokens": content.split_whitespace().count(),
                    "total_tokens": 200 + content.split_whitespace().count()
                }
            });
            return Ok(HttpResponse::Ok().json(resp));
        }

        // First turn — return tool_calls
        let tool_call = build_tool_call_response(&body, &model);
        return Ok(HttpResponse::Ok().json(tool_call));
    }

    // ── Non-streaming mode: return single JSON response ──
    if !wants_stream {
        let random_response = get_cached_file_response(&state, "zresponse").await?;
        // Truncate to reasonable length for non-streaming
        let content: String = random_response.chars().take(2000).collect();
        let resp = serde_json::json!({
            "id": generate_id(),
            "object": "chat.completion",
            "created": chrono::Utc::now().timestamp(),
            "model": &model,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": content
                },
                "logprobs": null,
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 50,
                "completion_tokens": content.split_whitespace().count(),
                "total_tokens": 50 + content.split_whitespace().count()
            }
        });
        return Ok(HttpResponse::Ok().json(resp));
    }

    // ── Streaming mode (default): SSE chunks ──
    let random_response = get_cached_file_response(&state, "zresponse").await?;

    let stream = openai_simulator(&random_response);

    let stream = stream.map(|chunk| {
        Ok::<_, actix_web::Error>(web::Bytes::from(chunk))
    });

    let model_clone = model.clone();
    let final_stream = stream.chain(futures_util::stream::once(async move {
        let final_chunk = Chunk {
            id: generate_id(),
            object: "chat.completion.chunk".to_string(),
            created: 1735278816,
            model: model_clone,
            system_fingerprint: "fp_d28bcae782".to_string(),
            choices: vec![],
            usage: Some(Usage {
                prompt_tokens: 182,
                completion_tokens: 520,
                total_tokens: 702,
                prompt_tokens_details: PromptTokensDetails { cached_tokens: 0, audio_tokens: 0 },
                completion_tokens_details: CompletionTokensDetails {
                    reasoning_tokens: 0,
                    audio_tokens: 0,
                    accepted_prediction_tokens: 0,
                    rejected_prediction_tokens: 0,
                },
            }),
        };

        let final_chunk_str = match serde_json::to_string(&final_chunk) {
            Ok(str) => str,
            Err(e) => {
                error!("Failed to serialize final chunk: {}", e);
                return Ok::<_, actix_web::Error>(web::Bytes::from("data: [ERROR]\n\n"));
            }
        };

        let combined_final_chunk = format!("data: {}\n\ndata: [DONE]\n\n", final_chunk_str);

        Ok::<_, actix_web::Error>(web::Bytes::from(combined_final_chunk))
    }));

    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .streaming(final_stream))
}

/// Build a tool_calls response when request contains tools.
/// Extracts the first tool name and generates a mock invocation.
fn build_tool_call_response(body: &OpenAiRequest, model: &str) -> serde_json::Value {
    let tools = body.tools.as_ref().unwrap();
    // Pick first tool that looks relevant
    let tool = &tools[0];
    let tool_name = tool["function"]["name"].as_str().unwrap_or("unknown");

    // Extract user message to build relevant arguments
    let user_msg = body.messages.as_ref()
        .and_then(|msgs| msgs.iter().rev().find(|m| m["role"] == "user"))
        .and_then(|m| m["content"].as_str())
        .unwrap_or("query");

    // Generate arguments based on tool name
    let arguments = match tool_name {
        "knowledge_base" | "search" => {
            serde_json::json!({"query": user_msg}).to_string()
        }
        "system_status" => {
            serde_json::json!({"service": "all"}).to_string()
        }
        "sla_calculator" => {
            serde_json::json!({"priority": "P2"}).to_string()
        }
        "calculator" => {
            serde_json::json!({"operation": "average", "values": [10.0, 20.0]}).to_string()
        }
        "fetch_products" => {
            serde_json::json!({"category": "all"}).to_string()
        }
        _ => {
            serde_json::json!({"input": user_msg}).to_string()
        }
    };

    serde_json::json!({
        "id": generate_id(),
        "object": "chat.completion",
        "created": chrono::Utc::now().timestamp(),
        "model": model,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": format!("call_{}", &generate_id()[9..]),
                    "type": "function",
                    "function": {
                        "name": tool_name,
                        "arguments": arguments
                    }
                }]
            },
            "logprobs": null,
            "finish_reason": "tool_calls"
        }],
        "usage": {
            "prompt_tokens": 100,
            "completion_tokens": 20,
            "total_tokens": 120
        }
    })
}

// =============================================================================
// Multi-Dialect Endpoints (Anthropic, Ollama, Cohere, Gemini)
// =============================================================================

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnthropicRequest {
    #[serde(default = "default_anthropic_model")]
    model: String,
    #[serde(default)]
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ChatMessage {
    #[serde(default)]
    role: String,
    #[serde(default)]
    content: String,
}

fn default_anthropic_model() -> String { "claude-3-5-sonnet-sim".to_string() }

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OllamaRequest {
    #[serde(default = "default_ollama_model")]
    model: String,
    #[serde(default)]
    messages: Vec<ChatMessage>,
}

fn default_ollama_model() -> String { "llama3-sim".to_string() }

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CohereRequest {
    #[serde(default)]
    messages: Vec<ChatMessage>,
}

#[actix_web::post("/v1/messages")]
async fn anthropic_messages(
    state: web::Data<Arc<AppState>>,
    semaphore: web::Data<Arc<Semaphore>>,
    body: web::Json<AnthropicRequest>,
) -> Result<HttpResponse, CustomError> {
    let _permit = semaphore.acquire().await.map_err(|_| CustomError::FetchError)?;
    info!("Received Anthropic messages request (model: {})", body.model);

    let random_response = get_cached_file_response(&state, "zresponse").await
        .unwrap_or_else(|_| "Based on the analysis, the key findings show strong performance.".to_string());

    let stream = anthropic_simulator(&random_response, &body.model);
    let stream = stream.map(|chunk| Ok::<_, actix_web::Error>(web::Bytes::from(chunk)));

    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .streaming(stream))
}

#[actix_web::post("/api/chat")]
async fn ollama_chat(
    state: web::Data<Arc<AppState>>,
    semaphore: web::Data<Arc<Semaphore>>,
    body: web::Json<OllamaRequest>,
) -> Result<HttpResponse, CustomError> {
    let _permit = semaphore.acquire().await.map_err(|_| CustomError::FetchError)?;
    info!("Received Ollama chat request (model: {})", body.model);

    let random_response = get_cached_file_response(&state, "zresponse").await
        .unwrap_or_else(|_| "Based on the analysis, the key findings show strong performance.".to_string());

    let stream = ollama_simulator(&random_response, &body.model);
    let stream = stream.map(|chunk| Ok::<_, actix_web::Error>(web::Bytes::from(chunk)));

    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .streaming(stream))
}

#[actix_web::post("/v1/chat")]
async fn cohere_chat(
    state: web::Data<Arc<AppState>>,
    semaphore: web::Data<Arc<Semaphore>>,
) -> Result<HttpResponse, CustomError> {
    let _permit = semaphore.acquire().await.map_err(|_| CustomError::FetchError)?;
    info!("Received Cohere chat request");

    let random_response = get_cached_file_response(&state, "zresponse").await
        .unwrap_or_else(|_| "Based on the analysis, the key findings show strong performance.".to_string());

    let stream = cohere_simulator(&random_response);
    let stream = stream.map(|chunk| Ok::<_, actix_web::Error>(web::Bytes::from(chunk)));

    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .streaming(stream))
}

#[actix_web::post("/v1beta/models/{model_action}")]
async fn gemini_stream(
    state: web::Data<Arc<AppState>>,
    semaphore: web::Data<Arc<Semaphore>>,
    path: web::Path<String>,
) -> Result<HttpResponse, CustomError> {
    let _permit = semaphore.acquire().await.map_err(|_| CustomError::FetchError)?;
    let model_action = path.into_inner();
    let model = model_action.split(':').next().unwrap_or("gemini-sim").to_string();
    info!("Received Gemini stream request (model: {})", model);

    let random_response = get_cached_file_response(&state, "zresponse").await
        .unwrap_or_else(|_| "Based on the analysis, the key findings show strong performance.".to_string());

    let stream = gemini_simulator(&random_response, &model);
    let stream = stream.map(|chunk| Ok::<_, actix_web::Error>(web::Bytes::from(chunk)));

    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .streaming(stream))
}

#[actix_web::main]
async fn main() -> Result<(), CustomError> {
    let log_level = match CONFIG.log_level.as_str() {
        "trace" => log::LevelFilter::Trace,
        "debug" => log::LevelFilter::Debug,
        "info" => log::LevelFilter::Info,
        "warn" => log::LevelFilter::Warn,
        "error" => log::LevelFilter::Error,
        _ => log::LevelFilter::Info,
    };

    Builder::new()
        .filter(None, log_level)
        .init();

    // Auto-free port if occupied
    let port = CONFIG.binding.port;
    if std::net::TcpListener::bind(("0.0.0.0", port)).is_err() {
        eprintln!("Port {} in use — releasing...", port);
        #[cfg(unix)]
        {
            let _ = std::process::Command::new("sh")
                .args(["-c", &format!("kill $(lsof -ti:{}) 2>/dev/null", port)])
                .output();
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }

    println!("AI Endpoint Simulator: http://{}:{} (workers={}, log={})",
        CONFIG.binding.host, port, CONFIG.workers, CONFIG.log_level);

    let app_state = Arc::new(AppState::new());
    let semaphore = Arc::new(Semaphore::new(CONFIG.semaphore_limit));

    HttpServer::new(move || {
        App::new()
            // Logger only when log_level is info or lower
            .wrap(actix_web::middleware::Condition::new(
                CONFIG.log_level == "info" || CONFIG.log_level == "debug" || CONFIG.log_level == "trace",
                Logger::default(),
            ))
            .app_data(web::Data::new(app_state.clone()))
            .app_data(web::Data::new(semaphore.clone()))
            .service(health_check)
            .service(chat_completions)
            .service(test_completion)
            .service(anthropic_messages)
            .service(ollama_chat)
            .service(cohere_chat)
            .service(gemini_stream)
    })
        .workers(CONFIG.workers)
        .bind(format!("{}:{}", CONFIG.binding.host, CONFIG.binding.port))?
        .run()
        .await
        .map_err(|e| CustomError::BindError(e.to_string()))
}
