//! # 音频质量分析器主程序
//!
//! 这是音频质量分析器的主入口点，提供命令行界面和用户交互功能。

use audio_analyzer_ultimate::{
    utils::{fs_utils, input_utils, Timer},
    AnalyzerConfig, AudioAnalyzer, Result,
};
use chrono::Local;
use clap::{Arg, Command as ClapCommand};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// 主程序入口点
fn main() -> Result<()> {
    // 解析命令行参数
    let matches = ClapCommand::new("audio-analyzer")
        .version("4.0.0")
        .author("Audio Analyzer Team")
        .about("高性能音频质量分析器")
        .long_about(
            "一个基于 Rust + Python 的高性能音频质量分析工具，支持批量处理和详细的质量评估报告。",
        )
        .arg(
            Arg::new("input")
                .help("要分析的音频文件或目录路径")
                .value_name("PATH")
                .index(1),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .help("输出目录路径")
                .value_name("DIR"),
        )
        .arg(
            Arg::new("threads")
                .short('j')
                .long("threads")
                .help("并行线程数")
                .value_name("NUM")
                .value_parser(clap::value_parser!(usize)),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("启用详细输出")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .help("静默模式，只显示错误")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .help("配置文件路径")
                .value_name("FILE"),
        )
        .arg(
            Arg::new("formats")
                .long("formats")
                .help("支持的音频格式列表")
                .value_name("EXT1,EXT2,...")
                .value_delimiter(','),
        )
        .arg(
            Arg::new("max-files")
                .long("max-files")
                .help("最多扫描的音频文件数")
                .value_name("NUM")
                .value_parser(clap::value_parser!(usize)),
        )
        .arg(
            Arg::new("max-depth")
                .long("max-depth")
                .help("扫描最大目录深度")
                .value_name("NUM")
                .value_parser(clap::value_parser!(usize)),
        )
        .arg(
            Arg::new("follow-links")
                .long("follow-links")
                .help("扫描时跟随符号链接")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("cross-filesystems")
                .long("cross-filesystems")
                .help("允许跨文件系统扫描（默认禁用）")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("no-magic-check")
                .long("no-magic-check")
                .help("禁用基于文件头魔数的输入校验")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("ffmpeg-timeout")
                .long("ffmpeg-timeout")
                .help("FFmpeg 命令超时时间（秒）")
                .value_name("SECONDS")
                .value_parser(clap::value_parser!(u64)),
        )
        .arg(
            Arg::new("ffmpeg-max-procs")
                .long("ffmpeg-max-procs")
                .help("FFmpeg 最大并发进程数")
                .value_name("NUM")
                .value_parser(clap::value_parser!(usize)),
        )
        .arg(
            Arg::new("stderr-max-bytes")
                .long("stderr-max-bytes")
                .help("单次 FFmpeg 命令 stderr 最大保留字节数")
                .value_name("BYTES")
                .value_parser(clap::value_parser!(usize)),
        )
        .arg(
            Arg::new("python-script")
                .long("python-script")
                .help("显式指定 Python 分析脚本路径（仅开发/调试用途）")
                .value_name("FILE"),
        )
        .get_matches();

    // 显示欢迎信息（除非是静默模式）
    if !matches.get_flag("quiet") {
        println!("🎵 音频质量分析器 v4.0 (重构优化版)");
        println!("开始时间: {}", Local::now().format("%Y-%m-%d %H:%M:%S"));
        println!();
    }

    // 创建配置
    let config = create_config_from_matches(&matches)?;

    // 创建分析器实例
    let mut analyzer = AudioAnalyzer::new(config)?;

    // 初始化依赖项
    if !matches.get_flag("quiet") {
        println!("🔧 正在初始化依赖项...");
    }
    analyzer.initialize_dependencies()?;

    // 获取输入路径
    let folder_path = if let Some(input_path) = matches.get_one::<String>("input") {
        let path = PathBuf::from(input_path);
        if !path.exists() {
            eprintln!("❌ 错误: 指定的路径不存在: {}", path.display());
            std::process::exit(1);
        }
        if !path.is_dir() {
            eprintln!("❌ 错误: 指定的路径不是目录: {}", path.display());
            std::process::exit(1);
        }
        path
    } else {
        input_utils::get_folder_path_from_user()?
    };

    if !matches.get_flag("quiet") {
        println!("📂 正在扫描文件夹: {}", folder_path.display());
    }

    // 分析目录中的音频文件
    let timer = Timer::new("总体分析");
    let results = analyzer.analyze_directory(&folder_path)?;

    if results.is_empty() {
        if !matches.get_flag("quiet") {
            println!("⚠️  在指定路径下没有找到支持的音频文件。");
            println!(
                "支持的格式: {}",
                analyzer.config().supported_extensions.join(", ")
            );
        }
        return Ok(());
    }

    if !matches.get_flag("quiet") {
        println!("\n✅ 数据提取完成");
        timer.print_elapsed();
        println!("📊 成功分析 {} 个文件", results.len());
    }

    // 保存中间数据到JSON文件
    let output_dir = if let Some(output) = matches.get_one::<String>("output") {
        PathBuf::from(output)
    } else {
        folder_path.clone()
    };
    fs_utils::ensure_dir_exists(&output_dir)?;

    let json_output_path = output_dir.join("analysis_data.json");
    if !matches.get_flag("quiet") {
        println!("💾 正在保存分析数据到: {}", json_output_path.display());
    }

    let json_content = serde_json::to_string_pretty(&results)?;
    fs::write(&json_output_path, json_content)?;

    if !matches.get_flag("quiet") {
        println!("✅ 分析数据保存成功");
    }

    // 调用Python分析模块生成最终报告
    let csv_output_path = output_dir.join("audio_quality_report.csv");
    call_python_analyzer(
        analyzer.get_analyzer_path().map(PathBuf::from),
        &json_output_path,
        &csv_output_path,
        matches.get_one::<String>("python-script"),
        matches.get_flag("quiet"),
    )?;

    if !matches.get_flag("quiet") {
        println!("\n🎉 分析流程完成");
        println!("📄 最终报告: {}", csv_output_path.display());
        println!("📄 原始数据: {}", json_output_path.display());
        println!("⏰ 结束时间: {}", Local::now().format("%Y-%m-%d %H:%M:%S"));
    }

    Ok(())
}

/// 从命令行参数创建配置
fn create_config_from_matches(matches: &clap::ArgMatches) -> Result<AnalyzerConfig> {
    let mut config = AnalyzerConfig::default();

    // 从配置文件加载（如果指定）
    if let Some(config_file) = matches.get_one::<String>("config") {
        config = AnalyzerConfig::from_file(config_file)?;
    }

    // 命令行参数覆盖配置文件设置
    if matches.get_flag("verbose") {
        config.verbose = true;
    }

    if matches.get_flag("quiet") {
        config.verbose = false;
        config.show_progress = false;
    }

    if let Some(&threads) = matches.get_one::<usize>("threads") {
        config.num_threads = Some(threads);
    }

    if let Some(formats) = matches.get_many::<String>("formats") {
        config.supported_extensions = formats.cloned().collect();
    }

    if let Some(&max_files) = matches.get_one::<usize>("max-files") {
        config.scan.max_files = Some(max_files);
    }

    if let Some(&max_depth) = matches.get_one::<usize>("max-depth") {
        config.scan.max_depth = Some(max_depth);
    }

    if matches.get_flag("follow-links") {
        config.scan.follow_links = true;
    }

    if matches.get_flag("cross-filesystems") {
        config.scan.same_file_system = false;
    }

    if matches.get_flag("no-magic-check") {
        config.scan.verify_magic_bytes = false;
    }

    if let Some(&timeout) = matches.get_one::<u64>("ffmpeg-timeout") {
        config.ffmpeg.timeout_seconds = Some(timeout);
    }

    if let Some(&max_procs) = matches.get_one::<usize>("ffmpeg-max-procs") {
        config.ffmpeg.max_parallel_processes = Some(max_procs);
    }

    if let Some(&stderr_max) = matches.get_one::<usize>("stderr-max-bytes") {
        config.ffmpeg.stderr_max_bytes = stderr_max;
    }

    // 从环境变量读取配置（优先级最低）
    if !matches.get_flag("verbose") && !matches.get_flag("quiet") {
        if let Ok(verbose) = std::env::var("AUDIO_ANALYZER_VERBOSE") {
            config.verbose = verbose.eq_ignore_ascii_case("true") || verbose == "1";
        }
    }

    if matches.get_one::<usize>("threads").is_none() {
        if let Ok(threads) = std::env::var("AUDIO_ANALYZER_THREADS") {
            if let Ok(num) = threads.parse::<usize>() {
                config.num_threads = Some(num);
            }
        }
    }

    if matches.get_one::<u64>("ffmpeg-timeout").is_none() {
        if let Ok(timeout) = std::env::var("AUDIO_ANALYZER_FFMPEG_TIMEOUT") {
            if let Ok(timeout_seconds) = timeout.parse::<u64>() {
                config.ffmpeg.timeout_seconds = Some(timeout_seconds);
            }
        }
    }

    if matches.get_one::<usize>("ffmpeg-max-procs").is_none() {
        if let Ok(max_procs) = std::env::var("AUDIO_ANALYZER_FFMPEG_MAX_PROCS") {
            if let Ok(value) = max_procs.parse::<usize>() {
                config.ffmpeg.max_parallel_processes = Some(value);
            }
        }
    }

    if matches.get_one::<usize>("stderr-max-bytes").is_none() {
        if let Ok(stderr_max) = std::env::var("AUDIO_ANALYZER_STDERR_MAX_BYTES") {
            if let Ok(value) = stderr_max.parse::<usize>() {
                config.ffmpeg.stderr_max_bytes = value;
            }
        }
    }

    if matches.get_one::<usize>("max-files").is_none() {
        if let Ok(max_files) = std::env::var("AUDIO_ANALYZER_MAX_FILES") {
            if let Ok(value) = max_files.parse::<usize>() {
                config.scan.max_files = Some(value);
            }
        }
    }

    if matches.get_one::<usize>("max-depth").is_none() {
        if let Ok(max_depth) = std::env::var("AUDIO_ANALYZER_MAX_DEPTH") {
            if let Ok(value) = max_depth.parse::<usize>() {
                config.scan.max_depth = Some(value);
            }
        }
    }

    // 默认设置
    if !matches.get_flag("quiet") {
        config.show_progress = true;
    }

    Ok(config)
}

/// 调用Python分析器生成最终报告
fn call_python_analyzer(
    embedded_analyzer_path: Option<PathBuf>,
    json_path: &PathBuf,
    csv_path: &PathBuf,
    python_script_override: Option<&String>,
    quiet: bool,
) -> Result<()> {
    if !quiet {
        println!("\n🐍 正在调用Python分析模块生成最终报告...");
    }

    if let Some(script_path) = python_script_override {
        let script = PathBuf::from(script_path);
        if !script.exists() {
            return Err(audio_analyzer_ultimate::AnalyzerError::Other(format!(
                "指定的 Python 脚本不存在: {}",
                script.display()
            )));
        }

        let mut command = Command::new("python3");
        command.arg(&script).arg(json_path).arg("-o").arg(csv_path);

        let status = command.status()?;

        if !status.success() {
            return Err(audio_analyzer_ultimate::AnalyzerError::Other(format!(
                "Python分析模块执行失败，退出代码: {:?}",
                status.code()
            )));
        }

        if !quiet {
            println!("✅ 指定 Python 脚本执行成功");
        }
        return Ok(());
    }

    if let Some(trusted_script) = default_python_script_path() {
        let status = Command::new("python3")
            .arg(&trusted_script)
            .arg(json_path)
            .arg("-o")
            .arg(csv_path)
            .status()?;

        if status.success() {
            if !quiet {
                println!("✅ 受信任 Python 脚本执行成功");
            }
            return Ok(());
        }

        if !quiet {
            println!(
                "⚠️  受信任 Python 脚本执行失败（退出码: {:?}），尝试内置分析器...",
                status.code()
            );
        }
    }

    if let Some(analyzer_path) = embedded_analyzer_path {
        if !analyzer_path.exists() {
            return Err(audio_analyzer_ultimate::AnalyzerError::DependencyError(
                format!("内置分析器不存在: {}", analyzer_path.display()),
            ));
        }

        let status = Command::new(&analyzer_path)
            .arg(json_path)
            .arg("-o")
            .arg(csv_path)
            .status()?;

        if !status.success() {
            return Err(audio_analyzer_ultimate::AnalyzerError::Other(format!(
                "内置分析器执行失败，退出代码: {:?}",
                status.code()
            )));
        }

        if !quiet {
            println!("✅ 内置分析器执行成功");
        }

        return Ok(());
    }

    Err(audio_analyzer_ultimate::AnalyzerError::DependencyError(
        "未获取到可执行的分析器路径".to_string(),
    ))
}

fn default_python_script_path() -> Option<PathBuf> {
    let manifest_script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("bin")
        .join("audio_analyzer.py");
    if manifest_script.exists() {
        return Some(manifest_script);
    }

    let executable_script = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.join("audio_analyzer.py")));
    executable_script.filter(|p| p.exists())
}

/// 显示使用帮助
#[allow(dead_code)]
fn show_help() {
    println!("音频质量分析器 v4.0");
    println!();
    println!("用法:");
    println!("  audio-analyzer [选项]");
    println!();
    println!("环境变量:");
    println!("  AUDIO_ANALYZER_VERBOSE=true    启用详细输出");
    println!("  AUDIO_ANALYZER_THREADS=4       设置并行线程数");
    println!();
    println!("支持的音频格式:");
    println!("  WAV, MP3, FLAC, AAC, OGG, OPUS, WMA, AIFF, ALAC, M4A");
    println!();
    println!("输出文件:");
    println!("  analysis_data.json           - 中间分析数据");
    println!("  audio_quality_report.csv     - 最终质量报告");
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Command as ClapCommand;

    #[test]
    fn test_create_config() {
        // 创建一个简单的测试配置，包含所有必需的参数
        let matches = ClapCommand::new("test")
            .arg(
                clap::Arg::new("verbose")
                    .long("verbose")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("quiet")
                    .long("quiet")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("threads")
                    .long("threads")
                    .value_parser(clap::value_parser!(usize)),
            )
            .arg(clap::Arg::new("config").long("config").value_name("FILE"))
            .arg(
                clap::Arg::new("formats")
                    .long("formats")
                    .value_delimiter(','),
            )
            .arg(
                clap::Arg::new("max-files")
                    .long("max-files")
                    .value_parser(clap::value_parser!(usize)),
            )
            .arg(
                clap::Arg::new("max-depth")
                    .long("max-depth")
                    .value_parser(clap::value_parser!(usize)),
            )
            .arg(
                clap::Arg::new("follow-links")
                    .long("follow-links")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("cross-filesystems")
                    .long("cross-filesystems")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("no-magic-check")
                    .long("no-magic-check")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("ffmpeg-timeout")
                    .long("ffmpeg-timeout")
                    .value_parser(clap::value_parser!(u64)),
            )
            .arg(
                clap::Arg::new("ffmpeg-max-procs")
                    .long("ffmpeg-max-procs")
                    .value_parser(clap::value_parser!(usize)),
            )
            .arg(
                clap::Arg::new("stderr-max-bytes")
                    .long("stderr-max-bytes")
                    .value_parser(clap::value_parser!(usize)),
            )
            .arg(clap::Arg::new("python-script").long("python-script"))
            .try_get_matches_from(vec!["test", "--verbose"])
            .unwrap();

        let config = create_config_from_matches(&matches).unwrap();
        assert!(config.verbose);
        // 验证默认配置
        assert!(config.show_progress); // 默认应该显示进度
    }
}
