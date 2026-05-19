use crate::config::LlmConfig;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f64,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

/// Events emitted during streaming
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// A new token from the LLM
    Token(String),
    /// Stream completed successfully (accumulated full text)
    Done(String),
    /// Stream failed
    Error(String),
}

/// Non-streaming chat (single turn)
pub async fn chat(
    config: &LlmConfig,
    system_prompt: &str,
    user_message: &str,
) -> anyhow::Result<String> {
    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: system_prompt.into(),
        },
        ChatMessage {
            role: "user".into(),
            content: user_message.into(),
        },
    ];
    chat_with_messages(config, &messages).await
}

/// Non-streaming chat with full message history
pub async fn chat_with_messages(
    config: &LlmConfig,
    messages: &[ChatMessage],
) -> anyhow::Result<String> {
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

    let body = ChatRequest {
        model: config.model.clone(),
        messages: messages.to_vec(),
        temperature: 0.8,
        stream: false,
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .with_context(|| format!("Failed to connect to LLM at {url}"))?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("LLM API error ({}): {text}", status.as_u16());
    }

    let data: ChatResponse = resp
        .json()
        .await
        .with_context(|| "Failed to parse LLM response")?;

    let content = data
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .unwrap_or_default();

    Ok(content)
}

/// Streaming chat — returns a receiver for token-by-token output
pub fn chat_stream(
    config: LlmConfig,
    messages: Vec<ChatMessage>,
) -> mpsc::UnboundedReceiver<StreamEvent> {
    let (tx, rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        let result = stream_impl(&config, &messages, &tx).await;
        if let Err(e) = result {
            let _ = tx.send(StreamEvent::Error(format!("{:#}", e)));
        }
    });

    rx
}

async fn stream_impl(
    config: &LlmConfig,
    messages: &[ChatMessage],
    tx: &mpsc::UnboundedSender<StreamEvent>,
) -> anyhow::Result<()> {
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

    let body = ChatRequest {
        model: config.model.clone(),
        messages: messages.to_vec(),
        temperature: 0.8,
        stream: true,
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .with_context(|| format!("Failed to connect to LLM at {url}"))?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("LLM API error ({}): {text}", status.as_u16());
    }

    let mut full_text = String::new();
    let mut stream = resp.bytes_stream();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Failed to read stream chunk")?;
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || !line.starts_with("data: ") {
                continue;
            }

            let data = &line[6..]; // strip "data: "
            if data == "[DONE]" {
                let _ = tx.send(StreamEvent::Done(full_text));
                return Ok(());
            }

            match serde_json::from_str::<StreamChunk>(data) {
                Ok(chunk) => {
                    if let Some(delta) = chunk
                        .choices
                        .first()
                        .and_then(|c| c.delta.content.as_deref())
                    {
                        if !delta.is_empty() {
                            full_text.push_str(delta);
                            let _ = tx.send(StreamEvent::Token(delta.to_string()));
                        }
                    }
                }
                Err(_) => {
                    // Skip unparseable lines
                }
            }
        }
    }

    // Stream ended without [DONE]
    let _ = tx.send(StreamEvent::Done(full_text));
    Ok(())
}
