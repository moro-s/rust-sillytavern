use crate::config::LlmConfig;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f64,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Tool>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String, // JSON string
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: Option<ChatMessage>,
    delta: Option<StreamDelta>,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatMessageResp {
    role: Option<String>,
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ToolCall>,
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
    /// Stream cancelled (accumulated partial text)
    Cancelled(String),
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
            content: Some(system_prompt.into()),
            tool_calls: None,
            tool_call_id: None,
        },
        ChatMessage {
            role: "user".into(),
            content: Some(user_message.into()),
            tool_calls: None,
            tool_call_id: None,
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
    log::debug!("LLM 非流式请求: url={}, model={}, messages={}", url, config.model, messages.len());

    let body = ChatRequest {
        model: config.model.clone(),
        messages: messages.to_vec(),
        temperature: 0.8,
        stream: false,
        tools: None,
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
        .and_then(|c| c.message)
        .and_then(|m| m.content)
        .unwrap_or_default();

    Ok(content)
}

/// Get the tools list
pub fn default_tools() -> Vec<Tool> {
    vec![
        manage_state_tool(),
        advance_time_tool(),
    ]
}

/// Get the manage_state tool
pub fn manage_state_tool() -> Tool {
    Tool {
        tool_type: "function".into(),
        function: ToolFunction {
            name: "manage_state".into(),
            description: "管理角色/世界/地点状态。增删改查物品、事件、技能、状态、法则。\n用于: 记录新物品、更新角色状态、查询世界信息等。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["get", "search", "add", "update", "delete"], "description": "get=获取, search=搜索, add=添加, update=更新, delete=删除"},
                    "category": {"type": "string", "enum": ["item", "event", "skill", "status", "rule"], "description": "item=物品, event=事件, skill=技能, status=当前状态, rule=世界法则"},
                    "key": {"type": "string", "description": "标识名（物品名/技能名/状态名等）"},
                    "data": {"type": "object", "description": "数据体（添加/更新时使用），如 item: {qty:1, note:\"描述\"}, event: {desc:\"...\", importance:\"high\"}, skill: {desc:\"...\", type:\"passive\"}, status: {detail:\"...\"}"}
                },
                "required": ["action", "category", "key"]
            }),
        },
    }
}

/// Get the advance_time tool
pub fn advance_time_tool() -> Tool {
    Tool {
        tool_type: "function".into(),
        function: ToolFunction {
            name: "advance_time".into(),
            description: "推进世界时间线到下一个时刻。用于剧情推进、跳过时间段、进入新场景。\n当剧情需要时间推进（如'第二天'、'一周后'）时调用。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "label": {"type": "string", "description": "时间标签，如 '第2天早晨', '三天后', '一周后的夜晚'"},
                    "description": {"type": "string", "description": "时间推进的描述/原因"}
                },
                "required": ["label"]
            }),
        },
    }
}

/// Chat with tools. The executor callback is called for each tool_call, returning the result string.
/// Loops until LLM returns a text response.
pub async fn chat_with_tools<F>(
    config: &LlmConfig,
    messages: &mut Vec<ChatMessage>,
    executor: F,
) -> anyhow::Result<String>
where
    F: Fn(&str, &str) -> String + Send,
{
    let tools = default_tools();
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();

    loop {
        let body = ChatRequest {
            model: config.model.clone(),
            messages: messages.clone(),
            temperature: 0.8,
            stream: false,
            tools: Some(tools.clone()),
        };

        let resp = client.post(&url)
            .header("Authorization", format!("Bearer {}", config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send().await
            .with_context(|| format!("Failed to connect to LLM at {url}"))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("LLM API error ({}): {text}", status.as_u16());
        }

        let data: ChatResponse = resp.json().await.with_context(|| "Failed to parse LLM response")?;
        let choice = data.choices.into_iter().next()
            .unwrap_or(ChatChoice { message: None, delta: None, finish_reason: None });

        let msg = choice.message;

        // If assistant has text content, return it
        if let Some(ref m) = msg {
            if let Some(ref content) = m.content {
                if !content.is_empty() {
                    // Add to messages for history
                    messages.push(ChatMessage {
                        role: "assistant".into(),
                        content: Some(content.clone()),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                    return Ok(content.clone());
                }
            }
        }

        // If assistant has tool calls, execute them
        if let Some(ref m) = msg {
            if let Some(ref calls) = m.tool_calls {
                // Push assistant message with tool_calls
                messages.push(ChatMessage {
                    role: "assistant".into(),
                    content: None,
                    tool_calls: Some(calls.clone()),
                    tool_call_id: None,
                });

                for call in calls {
                    if call.function.name == "manage_state" {
                        let args = &call.function.arguments;
                        let result = executor("manage_state", args);
                        // Push tool result
                        messages.push(ChatMessage {
                            role: "tool".into(),
                            content: Some(result),
                            tool_calls: None,
                            tool_call_id: Some(call.id.clone()),
                        });
                    }
                }
                continue; // Continue the loop to get final answer
            }
        }

        // No content and no tool calls - something unexpected
        break;
    }

    Ok(String::new())
}

/// Streaming chat — returns a receiver for token-by-token output.
/// Pass a `cancel` watch receiver to support interruption.
pub fn chat_stream(
    config: LlmConfig,
    messages: Vec<ChatMessage>,
    cancel: Option<tokio::sync::watch::Receiver<bool>>,
) -> mpsc::UnboundedReceiver<StreamEvent> {
    let (tx, rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        let result = stream_impl(&config, &messages, &tx, cancel).await;
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
    mut cancel: Option<tokio::sync::watch::Receiver<bool>>,
) -> anyhow::Result<()> {
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
    log::debug!("LLM 流式请求: url={}, model={}, messages={}", url, config.model, messages.len());

    let body = ChatRequest {
        model: config.model.clone(),
        messages: messages.to_vec(),
        temperature: 0.8,
        stream: true,
        tools: None,
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

    loop {
        // Check for cancellation before reading next chunk
        if let Some(ref mut cancel_rx) = cancel {
            if *cancel_rx.borrow() {
                let _ = tx.send(StreamEvent::Cancelled(full_text));
                return Ok(());
            }
            // Wait for either: cancel signal or next chunk
            let next = tokio::select! {
                biased;
                _ = cancel_rx.changed() => {
                    let _ = tx.send(StreamEvent::Cancelled(full_text));
                    return Ok(());
                }
                chunk = stream.next() => {
                    chunk
                }
            };
            let Some(chunk) = next else { break };
            let chunk = chunk.context("Failed to read stream chunk")?;
            let text = String::from_utf8_lossy(&chunk);

            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || !line.starts_with("data: ") {
                    continue;
                }

                let data = &line[6..];
                if data == "[DONE]" {
                    log::info!("LLM 流式完成, 共计 {} 字", full_text.chars().count());
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
                    Err(_) => {}
                }
            }
        } else {
            // No cancel token: simple loop
            let Some(chunk) = stream.next().await else { break };
            let chunk = chunk.context("Failed to read stream chunk")?;
            let text = String::from_utf8_lossy(&chunk);

            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || !line.starts_with("data: ") {
                    continue;
                }

                let data = &line[6..];
                if data == "[DONE]" {
                    log::info!("LLM 流式完成, 共计 {} 字", full_text.chars().count());
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
                    Err(_) => {}
                }
            }
        }
    }

    // Stream ended without [DONE]
    log::info!("LLM 流式完成(无DONE标记), 共计 {} 字", full_text.chars().count());
    let _ = tx.send(StreamEvent::Done(full_text));
    Ok(())
}
