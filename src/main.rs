mod character;
mod command;
mod config;
mod conversation;
mod db;
mod llm;
mod logging;
mod lorebook;
mod skill;
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

    // ── 关系图谱 ──
    /// 列出所有角色关系
    #[arg(long = "rgl", alias = "relation-list")]
    list_relations: bool,

    /// 按角色 slug 查找关系
    #[arg(long = "rgf", alias = "relation-find")]
    relation_find: Option<String>,

    /// 添加角色关系：FROM_SLUG TO_SLUG REL_TYPE AFFINITY [NOTE]
    #[arg(long = "rga", num_args = 4..=5)]
    relation_add: Vec<String>,

    // ── 任务系统 ──
    /// 列出某世界的任务（输入世界 slug）
    #[arg(long = "ql", alias = "quest-list")]
    quest_list: Option<String>,

    /// 查看任务详情（输入任务 ID）
    #[arg(long = "qt", alias = "quest-detail")]
    quest_detail: Option<i64>,

    /// 添加任务：TITLE DESC STATUS WORLD_SLUG
    #[arg(long = "qa", num_args = 4)]
    quest_add: Vec<String>,
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
    if args.list_relations {
        list_character_relations_cmd();
        return Ok(());
    }
    if let Some(slug) = args.relation_find {
        find_character_relations_cmd(&slug);
        return Ok(());
    }
    if !args.relation_add.is_empty() {
        add_character_relation_cmd(&args.relation_add);
        return Ok(());
    }
    if let Some(world_slug) = args.quest_list {
        list_quests_cmd(&world_slug);
        return Ok(());
    }
    if let Some(quest_id) = args.quest_detail {
        show_quest_detail_cmd(quest_id);
        return Ok(());
    }
    if !args.quest_add.is_empty() {
        add_quest_cmd(&args.quest_add);
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
        let mut system_prompt = format!(
            "你是一个角色扮演助手。请完全沉浸入以下角色的设定中，用角色的口吻回复。\n\n\
             【角色名】{}\n\n【性格】{}\n\n【说话风格】{}\n\n【开场白】{}\n\n\
             【重要规则】\n- 始终保持角色，不要跳出角色说话\n\
             - 不要代替用户说话或替用户做决定\n\
             - 回复时只输出角色的对话和动作/场景描写\n\
             - 动作描写用括号包裹，如 (放下酒杯)\n{}",
            card.name, card.personality, card.speech_style, card.first_message,
            if card.background.is_empty() { String::new() } else { format!("\n\n【背景知识】\n{}\n", card.background) },
        );

        // 注入 sys_skill/ 工具使用引导
        let skill_text = skill::load();
        if !skill_text.is_empty() {
            system_prompt.push_str(&format!("\n\n---\n【工具使用指南】\n{}", skill_text));
        }
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

// ──────────────────────────────────────────────
// 关系图谱命令处理
// ──────────────────────────────────────────────

fn list_character_relations_cmd() {
    match db::schema::open() {
        Ok(conn) => match db::store::list_character_relations(&conn) {
            Ok(relations) => {
                if relations.is_empty() {
                    println!("\n暂无角色关系\n");
                } else {
                    println!("\n角色关系谱:\n");
                    println!("{:<15} {:<6} {:<6} {:<15} {:<60}", "从", "→", "亲密度", "关系类型", "备注");
                    println!("{}", "-".repeat(110));
                    for r in &relations {
                        let affix = match r.affinity {
                            a if a > 50 => "❤",
                            a if a > 0 => "+",
                            0 => "·",
                            a if a > -50 => "-",
                            _ => "✗",
                        };
                        println!(
                            "{:<15} {:<6} {:>4} {:<6} {:<15} {:<60}",
                            truncate(&r.from_name, 14),
                            "→",
                            format!("{} {}", r.affinity, affix),
                            "",
                            truncate(&r.rel_type, 14),
                            truncate(&r.note, 58),
                        );
                    }
                    println!("\n共 {} 条关系\n", relations.len());
                }
            }
            Err(e) => eprintln!("列出关系失败: {}", e),
        },
        Err(e) => eprintln!("打开数据库失败: {}", e),
    }
}

fn find_character_relations_cmd(slug: &str) {
    match db::schema::open() {
        Ok(conn) => match db::store::find_character_relations(&conn, slug) {
            Ok(relations) => {
                if relations.is_empty() {
                    println!("\n未找到与 '{}' 相关的关系\n", slug);
                } else {
                    println!("\n'{}' 的角色关系:\n", slug);
                    for r in &relations {
                        let affix = match r.affinity {
                            a if a > 50 => "❤",
                            a if a > 0 => "+",
                            0 => "·",
                            a if a > -50 => "-",
                            _ => "✗",
                        };
                        println!(
                            "  {} → {} | 亲密度: {} {} | 类型: {} {}",
                            truncate(&r.from_name, 14),
                            truncate(&r.to_name, 14),
                            r.affinity,
                            affix,
                            truncate(&r.rel_type, 14),
                            if r.note.is_empty() { String::new() } else { format!("| 备注: {}", truncate(&r.note, 40)) },
                        );
                    }
                    println!();
                }
            }
            Err(e) => eprintln!("查找关系失败: {}", e),
        },
        Err(e) => eprintln!("打开数据库失败: {}", e),
    }
}

fn add_character_relation_cmd(args: &[String]) {
    if args.len() < 4 {
        eprintln!("用法: --rga <FROM_SLUG> <TO_SLUG> <REL_TYPE> <AFFINITY> [NOTE]");
        return;
    }
    let from_slug = &args[0];
    let to_slug = &args[1];
    let rel_type = &args[2];
    let affinity: i32 = match args[3].parse() {
        Ok(v) if (-100..=100).contains(&v) => v,
        _ => {
            eprintln!("亲密度必须为 -100 到 100 之间的整数");
            return;
        }
    };
    let note = args.get(4).map(|s| s.as_str()).unwrap_or("");

    match db::schema::open() {
        Ok(conn) => {
            let from = match db::store::get_character(&conn, from_slug) {
                Ok(Some(c)) => c.id,
                Ok(None) => { eprintln!("角色 '{}' 不存在", from_slug); return; }
                Err(e) => { eprintln!("数据库错误: {}", e); return; }
            };
            let to = match db::store::get_character(&conn, to_slug) {
                Ok(Some(c)) => c.id,
                Ok(None) => { eprintln!("角色 '{}' 不存在", to_slug); return; }
                Err(e) => { eprintln!("数据库错误: {}", e); return; }
            };
            match db::store::create_character_relation(&conn, from, to, rel_type, affinity, note) {
                Ok(()) => println!("已添加关系: {} → {} ({} 亲密度 {})", from_slug, to_slug, rel_type, affinity),
                Err(e) => eprintln!("添加关系失败: {}", e),
            }
        }
        Err(e) => eprintln!("打开数据库失败: {}", e),
    }
}

// ──────────────────────────────────────────────
// 任务系统命令处理
// ──────────────────────────────────────────────

fn list_quests_cmd(world_slug: &str) {
    match db::schema::open() {
        Ok(conn) => {
            let world_id = match db::store::get_world(&conn, world_slug) {
                Ok(Some(w)) => w.id,
                Ok(None) => { eprintln!("世界 '{}' 不存在", world_slug); return; }
                Err(e) => { eprintln!("数据库错误: {}", e); return; }
            };
            match db::store::list_quests(&conn, world_id) {
                Ok(quests) => {
                    if quests.is_empty() {
                        println!("\n世界 '{}' 暂无任务\n", world_slug);
                    } else {
                        println!("\n世界 '{}' 的任务列表:\n", world_slug);
                        println!("{:<6} {:<20} {:<12} {:<50}", "ID", "标题", "状态", "描述");
                        println!("{}", "-".repeat(92));
                        for q in &quests {
                            let status_icon = match q.status.as_str() {
                                "active" => "● 进行中",
                                "completed" => "✓ 已完成",
                                "failed" => "✗ 已失败",
                                _ => &q.status,
                            };
                            println!(
                                "{:<6} {:<20} {:<12} {:<50}",
                                q.id,
                                truncate(&q.title, 19),
                                status_icon,
                                truncate(&q.description, 48),
                            );
                        }
                        println!("\n共 {} 个任务\n", quests.len());
                    }
                }
                Err(e) => eprintln!("列出任务失败: {}", e),
            }
        }
        Err(e) => eprintln!("打开数据库失败: {}", e),
    }
}

fn show_quest_detail_cmd(quest_id: i64) {
    match db::schema::open() {
        Ok(conn) => match db::store::get_quest(&conn, quest_id) {
            Ok(Some((quest, members))) => {
                let status_str = match quest.status.as_str() {
                    "active" => "进行中",
                    "completed" => "已完成",
                    "failed" => "已失败",
                    s => s,
                };
                println!("\n═══ 任务详情 ═══");
                println!("  ID:      {}", quest.id);
                println!("  标题:    {}", quest.title);
                println!("  状态:    {}", status_str);
                println!("  描述:    {}", quest.description);
                if let Some(ref wn) = quest.world_name {
                    println!("  所属世界: {}", wn);
                }
                println!("  创建:    {}", &quest.created_at[..quest.created_at.len().min(16)]);
                if !members.is_empty() {
                    println!("\n  参与角色:");
                    for m in &members {
                        let role_icon = match m.role.as_str() {
                            "leader" => "👑",
                            "member" => "👤",
                            "giver" => "📜",
                            _ => "  ",
                        };
                        println!(
                            "    {} {} ({}) {}",
                            role_icon,
                            truncate(&m.character_name, 14),
                            truncate(&m.role, 10),
                            if m.task.is_empty() { String::new() } else { format!("任务: {}", truncate(&m.task, 30)) },
                        );
                    }
                }
                println!();
            }
            Ok(None) => println!("\n任务 ID {} 不存在\n", quest_id),
            Err(e) => eprintln!("查询任务失败: {}", e),
        },
        Err(e) => eprintln!("打开数据库失败: {}", e),
    }
}

fn add_quest_cmd(args: &[String]) {
    if args.len() < 4 {
        eprintln!("用法: --qa <TITLE> <DESCRIPTION> <STATUS> <WORLD_SLUG>");
        return;
    }
    let title = &args[0];
    let description = &args[1];
    let status = &args[2];
    let world_slug = &args[3];

    if !["active", "completed", "failed"].contains(&status.as_str()) {
        eprintln!("状态必须为: active / completed / failed");
        return;
    }

    match db::schema::open() {
        Ok(conn) => {
            let world_id = match db::store::get_world(&conn, world_slug) {
                Ok(Some(w)) => Some(w.id),
                Ok(None) => { eprintln!("世界 '{}' 不存在", world_slug); return; }
                Err(e) => { eprintln!("数据库错误: {}", e); return; }
            };
            match db::store::create_quest(&conn, title, description, status, world_id) {
                Ok(id) => println!("已创建任务 #{}: {} (状态: {})", id, title, status),
                Err(e) => eprintln!("创建任务失败: {}", e),
            }
        }
        Err(e) => eprintln!("打开数据库失败: {}", e),
    }
}
