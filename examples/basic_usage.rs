//! # 基本使用示例
//!
//! 演示如何使用音频分析器库进行基本的音频质量分析

use audio_analyzer_ultimate::{utils::Timer, AnalyzerConfig, AudioAnalyzer, Result};
use std::path::Path;

fn main() -> Result<()> {
    println!("音频质量分析器 - 基本使用示例");
    println!("================================");

    // 示例1: 使用默认配置
    basic_analysis_example()?;

    // 示例2: 自定义配置
    custom_config_example()?;

    // 示例3: 分析单个文件
    single_file_example()?;

    // 示例4: 批量分析
    batch_analysis_example()?;

    Ok(())
}

/// 示例1: 基本分析流程
fn basic_analysis_example() -> Result<()> {
    println!("\n📁 示例1: 基本分析流程");
    println!("{}", "-".repeat(40));

    // 创建默认配置的分析器
    let mut analyzer = AudioAnalyzer::with_default_config()?;

    // 初始化依赖项
    println!("正在初始化依赖项...");
    analyzer.initialize_dependencies()?;

    println!("✅ 分析器初始化完成");
    println!(
        "📊 支持的音频格式: {:?}",
        analyzer.config().supported_extensions
    );

    Ok(())
}

/// 示例2: 自定义配置
fn custom_config_example() -> Result<()> {
    println!("\n⚙️ 示例2: 自定义配置");
    println!("{}", "-".repeat(40));

    // 创建自定义配置
    let mut config = AnalyzerConfig {
        verbose: true,
        show_progress: true,
        num_threads: Some(4),
        supported_extensions: vec!["wav".to_string(), "flac".to_string(), "mp3".to_string()],
        ..Default::default()
    };

    // 自定义质量阈值
    config.quality_thresholds.lra_excellent_min = 10.0;
    config.quality_thresholds.lra_excellent_max = 15.0;

    println!("📋 自定义配置:");
    println!("  - 详细输出: {}", config.verbose);
    println!("  - 线程数: {:?}", config.num_threads);
    println!("  - 支持格式: {:?}", config.supported_extensions);
    println!(
        "  - LRA优秀范围: {}-{} LU",
        config.quality_thresholds.lra_excellent_min, config.quality_thresholds.lra_excellent_max
    );

    // 验证配置
    config.validate()?;
    println!("✅ 配置验证通过");

    // 创建分析器
    let mut analyzer = AudioAnalyzer::new(config)?;
    analyzer.initialize_dependencies()?;

    println!("✅ 自定义分析器创建完成");

    Ok(())
}

/// 示例3: 分析单个文件
fn single_file_example() -> Result<()> {
    println!("\n🎵 示例3: 分析单个文件");
    println!("{}", "-".repeat(40));

    // 创建分析器
    let mut analyzer = AudioAnalyzer::with_default_config()?;
    analyzer.initialize_dependencies()?;

    // 模拟文件路径（实际使用时替换为真实路径）
    let sample_file = Path::new("examples/sample.wav");

    if sample_file.exists() {
        println!("📂 分析文件: {}", sample_file.display());

        let timer = Timer::new("单文件分析");

        // 分析文件
        match analyzer.analyze_file(sample_file) {
            Ok(metrics) => {
                println!("✅ 分析完成!");
                print_metrics_summary(&metrics);
            }
            Err(e) => {
                println!("❌ 分析失败: {e}");
            }
        }

        timer.print_elapsed();
    } else {
        println!("⚠️  示例文件不存在: {}", sample_file.display());
        println!("   请将音频文件放在 examples/sample.wav 来测试此功能");
    }

    Ok(())
}

/// 示例4: 批量分析
fn batch_analysis_example() -> Result<()> {
    println!("\n📁 示例4: 批量分析");
    println!("{}", "-".repeat(40));

    // 创建分析器
    let mut analyzer = AudioAnalyzer::with_default_config()?;
    analyzer.initialize_dependencies()?;

    // 模拟目录路径
    let sample_dir = Path::new("examples/audio_samples");

    if sample_dir.exists() {
        println!("📂 分析目录: {}", sample_dir.display());

        let timer = Timer::new("批量分析");

        // 分析目录
        match analyzer.analyze_directory(sample_dir) {
            Ok(results) => {
                println!("✅ 批量分析完成!");
                println!("📊 处理了 {} 个文件", results.len());

                // 显示统计信息
                print_batch_summary(&results);
            }
            Err(e) => {
                println!("❌ 批量分析失败: {e}");
            }
        }

        timer.print_elapsed();
    } else {
        println!("⚠️  示例目录不存在: {}", sample_dir.display());
        println!("   请创建 examples/audio_samples/ 目录并放入音频文件来测试此功能");
    }

    Ok(())
}

/// 打印单个文件的分析结果摘要
fn print_metrics_summary(metrics: &audio_analyzer_ultimate::AudioMetrics) {
    println!("📊 分析结果摘要:");
    println!("  - 文件: {}", metrics.filename());
    println!(
        "  - 大小: {:.2} MB",
        metrics.file_size_bytes as f64 / 1024.0 / 1024.0
    );

    if let Some(lra) = metrics.lra {
        println!("  - LRA: {lra:.1} LU");
    }

    if let Some(peak) = metrics.peak_amplitude_db {
        println!("  - 峰值: {peak:.1} dB");
    }

    if let Some(rms_18k) = metrics.rms_db_above_18k {
        println!("  - 18kHz以上RMS: {rms_18k:.1} dB");
    }

    println!("  - 处理时间: {} ms", metrics.processing_time_ms);
    println!(
        "  - 数据完整性: {}",
        if metrics.is_complete() {
            "✅ 完整"
        } else {
            "⚠️ 不完整"
        }
    );
}

/// 打印批量分析结果摘要
fn print_batch_summary(results: &[audio_analyzer_ultimate::AudioMetrics]) {
    if results.is_empty() {
        println!("📊 没有找到可分析的文件");
        return;
    }

    let total_files = results.len();
    let complete_files = results.iter().filter(|m| m.is_complete()).count();
    let total_size: u64 = results.iter().map(|m| m.file_size_bytes).sum();
    let total_time: u64 = results.iter().map(|m| m.processing_time_ms).sum();

    println!("📊 批量分析统计:");
    println!("  - 总文件数: {total_files}");
    println!(
        "  - 完整分析: {} ({:.1}%)",
        complete_files,
        complete_files as f64 / total_files as f64 * 100.0
    );
    println!("  - 总大小: {:.2} MB", total_size as f64 / 1024.0 / 1024.0);
    println!("  - 总处理时间: {:.2} 秒", total_time as f64 / 1000.0);
    println!(
        "  - 平均处理时间: {:.0} ms/文件",
        total_time as f64 / total_files as f64
    );

    // LRA 统计
    let lra_values: Vec<f64> = results.iter().filter_map(|m| m.lra).collect();

    if !lra_values.is_empty() {
        let avg_lra = lra_values.iter().sum::<f64>() / lra_values.len() as f64;
        let min_lra = lra_values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_lra = lra_values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

        println!("  - LRA统计: 平均 {avg_lra:.1} LU, 范围 {min_lra:.1}-{max_lra:.1} LU");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_analyzer_creation() {
        let result = AudioAnalyzer::with_default_config();
        assert!(result.is_ok());
    }

    #[test]
    fn test_custom_config_validation() {
        let mut config = AnalyzerConfig::default();
        config.num_threads = Some(4);
        config.verbose = true;

        assert!(config.validate().is_ok());

        let analyzer = AudioAnalyzer::new(config);
        assert!(analyzer.is_ok());
    }
}
