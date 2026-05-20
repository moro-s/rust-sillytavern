use anyhow::Context;
use std::path::Path;

/// 初始化日志系统
///
/// 日志写入 `data/logs/` 目录，按天轮转，保留最近 7 天。
/// 默认级别可通过环境变量 `RUST_LOG` 控制，未设置则使用 `info,rust_sillytavern=debug`。
pub fn init() -> anyhow::Result<()> {
    let log_dir = Path::new("data").join("logs");
    std::fs::create_dir_all(&log_dir)
        .with_context(|| format!("无法创建日志目录: {}", log_dir.display()))?;

    flexi_logger::Logger::try_with_env_or_str("info,rust_sillytavern=debug")
        .with_context(|| "日志级别解析失败")?
        .log_to_file(
            flexi_logger::FileSpec::default()
                .directory(&log_dir)
                .basename("tavern"),
        )
        .rotate(
            flexi_logger::Criterion::Age(flexi_logger::Age::Day),
            flexi_logger::Naming::Timestamps,
            flexi_logger::Cleanup::KeepLogFiles(7),
        )
        .start()
        .with_context(|| "日志系统启动失败")?;

    log::info!("日志系统已初始化，日志目录: {}", log_dir.display());
    Ok(())
}
