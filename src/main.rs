mod character;
mod command;
mod config;
mod conversation;
mod db;
mod llm;
mod lorebook;
mod state;
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

fn scan_characters() -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir("characters") {
        names = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "md"))
            .filter_map(|e| {
                e.path()
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(String::from)
            })
            .collect();
        names.sort();
    }
    names
}

fn scan_worlds() -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let dirs = ["lorebooks", "worlds"];
    for dir in &dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            names.extend(
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().map_or(false, |ext| ext == "toml"))
                    .filter_map(|e| {
                        e.path()
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .map(String::from)
                    }),
            );
        }
    }
    names.sort();
    names.dedup();
    names
}

fn list_available_characters() {
    let names = scan_characters();
    println!("\n可用角色:\n");
    if names.is_empty() {
        println!("  (无角色文件)");
    } else {
        for name in &names {
            println!("  - {}", name);
        }
    }
    println!("\n共 {} 个角色\n", names.len());
}

fn list_available_worlds() {
    println!("\n可用世界（Lorebook）:\n");
    let names = scan_worlds();
    if names.is_empty() {
        println!("  lorebooks/ 或 worlds/ 目录不存在，请先创建\n");
    } else {
        for name in &names {
            println!("  - {}", name);
        }
        println!("\n共 {} 个世界\n", names.len());
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
        let card = character::load(char_name)?;
        let system_prompt = character::build_system_prompt(&card);

        if let Some(ref world) = args.world {
            println!("[世界: {}]", world);
        }
        println!("\n[{}]", card.meta.name);

        if cfg.llm.stream {
            let messages = vec![
                llm::ChatMessage { role: "system".into(), content: system_prompt },
                llm::ChatMessage { role: "user".into(), content: msg },
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
