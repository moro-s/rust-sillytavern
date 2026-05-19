mod character;
mod config;
mod llm;
mod tui;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "rust-SillyTavern")]
#[command(about = "AI role-playing tavern in terminal", long_about = None)]
struct Args {
    /// 角色名（对应 characters/ 目录下的 .md 文件，不含扩展名）
    #[arg(short = 'c', long = "char", default_value = "innkeeper")]
    character: String,

    /// 发给角色的消息（CLI 模式；不填则进入 TUI 交互模式）
    #[arg(short, long)]
    message: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if let Some(msg) = args.message {
        // CLI mode
        let cfg = config::load()?;
        let card = character::load(&args.character)?;
        let system_prompt = character::build_system_prompt(&card);

        println!("\n[{}]", card.meta.name);
        let reply = llm::chat(&cfg.llm, &system_prompt, &msg).await?;
        println!("{}\n", reply);
    } else {
        // TUI mode
        tui::app::run(&args.character).await?;
    }

    Ok(())
}
