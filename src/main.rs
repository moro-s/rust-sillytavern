mod character;
mod command;
mod config;
mod conversation;
mod db;
mod llm;
mod logging;
mod lorebook;
mod tui;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "rust-SillyTavern")]
#[command(about = "AI role-playing tavern in terminal", long_about = None)]
struct Args {
    /// 角色名（对应 characters/ 目录下的 .md 文件，不含扩展名；不填则启动选择器）
    #[arg(short = 'c', long = "char")]
    character: Option<String>,

    /// 世界名（预留：对应 lorebooks/ 或 worlds/ 目录）
    #[arg(short = 'w', long = "world")]
    world: Option<String>,

    /// 列出所有可用角色
    #[arg(long = "cl", alias = "char-list")]
    list_characters: bool,

    /// 列出所有可用世界
    #[arg(long = "wl", alias = "world-list")]
    list_worlds: bool,

    /// 发给角色的消息（CLI 模式；不填则进入 TUI 交互模式）
    #[arg(short, long)]
    message: Option<String>,

    /// 开始新会话（不恢复上次会话）
    #[arg(long = "new-session")]
    new_session: bool,

    /// 列出所有历史会话
    #[arg(long = "ls", alias = "list-sessions")]
    list_sessions: bool,

    /// 恢复指定会话 ID
    #[arg(long = "resume")]
    resume_id: Option<i64>,
}

fn list_available_characters() {
    if let Ok(db) = db::schema::open() {
        if let Ok(chars) = db::store::list_characters(&db) {
            println!("\n可用角色:\n");
            for c in &chars { println!("  - {} ({})", c.slug, c.name); }
            println!("\n共 {} 个角色\n", chars.len());
            return;
        }
    }
    println!("\n无法读取角色列表\n");
}

fn list_available_worlds() {
    if let Ok(db) = db::schema::open() {
        if let Ok(worlds) = db::store::list_worlds(&db) {
            println!("\n可用世界:\n");
            for w in &worlds { println!("  - {}", w.slug); }
            println!("\n共 {} 个世界\n", worlds.len());
            return;
        }
    }
    println!("\n无法读取世界列表\n");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logging::init()?;

    let args = Args::parse();

    // List commands — print and exit
    if args.list_characters {
        list_available_characters();
        return Ok(());
    }
    if args.list_worlds {
        list_available_worlds();
        return Ok(());
    }
    if args.list_sessions {
        list_saved_sessions();
        return Ok(());
    }

    if let Some(msg) = args.message {
        // CLI mode: use specified character or default
        let char_name = args.character.as_deref().unwrap_or("innkeeper");
        let cfg = config::load()?;
        // Use DB for character info in CLI mode
        let db = db::schema::open()?;
        let card = db::store::get_character(&db, char_name)?
            .ok_or_else(|| anyhow::anyhow!("Character '{}' not found in database", char_name))?;
        let system_prompt = format!(
            "你是一个角色扮演助手。请完全沉浸入以下角色的设定中，用角色的口吻回复。\n\n\
             【角色名】{}\n\n【性格】{}\n\n【说话风格】{}\n\n【开场白】{}\n\n\
             【重要规则】\n- 始终保持角色，不要跳出角色说话\n\
             - 不要代替用户说话或替用户做决定\n\
             - 回复时只输出角色的对话和动作/场景描写\n\
             - 动作描写用括号包裹，如 (放下酒杯)\n{}",
            card.name, card.personality, card.speech_style, card.first_message,
            if card.background.is_empty() { String::new() } else { format!("\n\n【背景知识】\n{}\n", card.background) },
        );
        println!("\n[{}]", card.name);

        if cfg.llm.stream {
            let messages = vec![
                llm::ChatMessage { role: "system".into(), content: Some(system_prompt), tool_calls: None, tool_call_id: None },
                llm::ChatMessage { role: "user".into(), content: Some(msg), tool_calls: None, tool_call_id: None },
            ];
            let mut rx = llm::chat_stream(cfg.llm, messages, None);
            while let Some(event) = rx.recv().await {
                match event {
                    llm::StreamEvent::Token(t) => print!("{}", t),
                    llm::StreamEvent::Done(_) => { println!(); break; }
                    llm::StreamEvent::Cancelled(_) => { println!("\n[已取消]"); break; }
                    llm::StreamEvent::Error(e) => { eprintln!("\nError: {}", e); break; }
                }
            }
        } else {
            let reply = llm::chat(&cfg.llm, &system_prompt, &msg).await?;
            println!("{}", reply);
        }
        println!();
    } else {
        // TUI mode
        tui::app::run(args.character, args.world, args.resume_id, args.new_session).await?;
    }

    Ok(())
}

fn list_saved_sessions() {
    match db::schema::open() {
        Ok(conn) => {
            match db::store::list_sessions(&conn) {
                Ok(sessions) => {
                    if sessions.is_empty() {
                        println!("\n暂无保存的会话\n");
                    } else {
                        println!("\n已保存的会话:\n");
                        for s in &sessions {
                            println!(
                                "  [{id}] {name:20} | {char:15} | {count:3} 条消息 | {time}",
                                id = s.id,
                                name = truncate(&s.name, 20),
                                char = truncate(&s.character_name, 15),
                                count = s.message_count,
                                time = &s.updated_at[..s.updated_at.len().min(16)],
                            );
                        }
                        println!("\n共 {} 个会话\n", sessions.len());
                    }
                }
                Err(e) => eprintln!("Failed to list sessions: {}", e),
            }
        }
        Err(e) => eprintln!("Failed to open database: {}", e),
    }
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() > max {
        format!("{}...", s.chars().take(max - 3).collect::<String>())
    } else {
        s.to_string()
    }
}
