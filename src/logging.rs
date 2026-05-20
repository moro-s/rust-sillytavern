use anyhow::Context;
use std::path::Path;

/// 初始化日志系统
///
/// 日志写入 `data/logs/` 目录：
/// - 活跃日志固定为 `tavern.log`（每次启动追加写入）
/// - 按天轮转或单文件超过 100MB 时自动归档为 `tavern_r<时间戳>.log`
/// - 保留最近 7 个归档文件
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
                .suppress_timestamp()
                .basename("tavern"),
        )
        .append()
        .rotate(
            flexi_logger::Criterion::AgeOrSize(flexi_logger::Age::Day, 100 * 1024 * 1024),
            flexi_logger::Naming::Timestamps,
            flexi_logger::Cleanup::KeepLogFiles(7),
        )
        .start()
        .with_context(|| "日志系统启动失败")?;

    log::info!("日志系统已初始化，日志目录: {}", log_dir.display());
    Ok(())
}
