//! # 音频质量分析器主程序
//!
//! 这是音频质量分析器的主入口点，提供命令行界面和用户交互功能。

use audio_analyzer_ultimate::{
    utils::{input_utils, Timer},
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
        &json_output_path,
        &csv_output_path,
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

    // 默认设置
    if !matches.get_flag("quiet") {
        config.show_progress = true;
    }

    Ok(config)
}

/// 调用Python分析器生成最终报告
fn call_python_analyzer(json_path: &PathBuf, csv_path: &PathBuf, quiet: bool) -> Result<()> {
    if !quiet {
        println!("\n🐍 正在调用Python分析模块生成最终报告...");
    }

    // 尝试使用系统中的Python分析器
    let python_script_path = std::env::current_dir()?
        .join("src")
        .join("bin")
        .join("audio_analyzer.py");

    if python_script_path.exists() {
        let mut command = Command::new("python3");
        command
            .arg(&python_script_path)
            .arg(json_path)
            .arg("-o")
            .arg(csv_path);

        let status = command.status()?;

        if !status.success() {
            return Err(audio_analyzer_ultimate::AnalyzerError::Other(format!(
                "Python分析模块执行失败，退出代码: {:?}",
                status.code()
            )));
        }

        if !quiet {
            println!("✅ Python分析模块执行成功");
        }
    } else if !quiet {
        println!("⚠️  警告: 未找到Python分析模块，跳过最终报告生成");
        println!("📄 中间数据已保存到: {}", json_path.display());
    }

    Ok(())
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
            .try_get_matches_from(vec!["test", "--verbose"])
            .unwrap();

        let config = create_config_from_matches(&matches).unwrap();
        assert!(config.verbose);
        // 验证默认配置
        assert!(config.show_progress); // 默认应该显示进度
    }
}
