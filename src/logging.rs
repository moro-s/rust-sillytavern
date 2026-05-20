use anyhow::Context;
use std::io::Write;
use std::path::Path;

/// 初始化日志系统
///
/// 日志写入 `data/logs/` 目录，命名格式 `年月日.log`（活跃）→ `年月日_r00001.log`（归档）：
/// - 按天轮转或单文件超过 100MB 时自动归档，序号递增
/// - 保留最近 7 个归档文件
/// 默认级别可通过环境变量 `RUST_LOG` 控制，未设置则使用 `info,rust_sillytavern=debug`。
pub fn init() -> anyhow::Result<()> {
    let log_dir = Path::new("data").join("logs");
    std::fs::create_dir_all(&log_dir)
        .with_context(|| format!("无法创建日志目录: {}", log_dir.display()))?;

    // 以当天日期（年月日）作为日志文件基准名，如 "20260520"
    let today = chrono::Local::now().format("%Y%m%d").to_string();

    flexi_logger::Logger::try_with_env_or_str("info,rust_sillytavern=debug")
        .with_context(|| "日志级别解析失败")?
        .log_to_file(
            flexi_logger::FileSpec::default()
                .directory(&log_dir)
                .suppress_timestamp()
                .basename(&today),
        )
        .append()
        .format(|w, now, record| {
            write!(
                w,
                "{} {} {}",
                record.module_path().unwrap_or("?"),
                now.format("%H:%M:%S"),
                record.args()
            )
        })
        .rotate(
            flexi_logger::Criterion::AgeOrSize(flexi_logger::Age::Day, 100 * 1024 * 1024),
            flexi_logger::Naming::Numbers,
            flexi_logger::Cleanup::KeepLogFiles(7),
        )
        .start()
        .with_context(|| "日志系统启动失败")?;

    log::info!("日志系统已初始化，日志目录: {}", log_dir.display());
    Ok(())
}
