//! # 配置模块单元测试
//!
//! 测试配置管理功能的正确性

use audio_analyzer_ultimate::config::{AnalyzerConfig, FfmpegConfig, OutputConfig};
use audio_analyzer_ultimate::types::QualityThresholds;
use tempfile::NamedTempFile;

#[test]
fn test_default_config_creation() {
    let config = AnalyzerConfig::default();

    // 验证默认值
    assert!(!config.supported_extensions.is_empty());
    assert!(config.supported_extensions.contains(&"wav".to_string()));
    assert!(config.supported_extensions.contains(&"mp3".to_string()));
    assert!(config.supported_extensions.contains(&"flac".to_string()));

    assert_eq!(config.num_threads, None);
    assert!(!config.verbose);
    assert!(config.show_progress);
}

#[test]
fn test_config_validation() {
    let mut config = AnalyzerConfig::default();

    // 有效配置应该通过验证
    assert!(config.validate().is_ok());

    // 空的扩展名列表应该失败
    config.supported_extensions.clear();
    assert!(config.validate().is_err());

    // 恢复扩展名列表
    config.supported_extensions = vec!["wav".to_string()];
    assert!(config.validate().is_ok());

    // 零线程数应该失败
    config.num_threads = Some(0);
    assert!(config.validate().is_err());

    // 正常线程数应该通过
    config.num_threads = Some(4);
    assert!(config.validate().is_ok());
}

#[test]
fn test_supported_extension_check() {
    let config = AnalyzerConfig::default();

    // 测试支持的扩展名
    assert!(config.is_supported_extension("wav"));
    assert!(config.is_supported_extension("WAV"));
    assert!(config.is_supported_extension("Mp3"));
    assert!(config.is_supported_extension("FLAC"));

    // 测试不支持的扩展名
    assert!(!config.is_supported_extension("txt"));
    assert!(!config.is_supported_extension("doc"));
    assert!(!config.is_supported_extension(""));
}

#[test]
fn test_effective_thread_count() {
    let mut config = AnalyzerConfig::default();

    // 默认情况下应该使用系统CPU数量
    let default_threads = config.effective_thread_count();
    assert!(default_threads > 0);
    assert!(default_threads <= num_cpus::get());

    // 设置特定线程数
    config.num_threads = Some(2);
    assert_eq!(config.effective_thread_count(), 2);

    config.num_threads = Some(8);
    assert_eq!(config.effective_thread_count(), 8);
}

#[test]
fn test_config_serialization() {
    let config = AnalyzerConfig::default();
    let temp_file = NamedTempFile::new().unwrap();

    // 保存配置
    config.save_to_file(temp_file.path()).unwrap();

    // 加载配置
    let loaded_config = AnalyzerConfig::from_file(temp_file.path()).unwrap();

    // 验证配置相同
    assert_eq!(
        config.supported_extensions,
        loaded_config.supported_extensions
    );
    assert_eq!(config.verbose, loaded_config.verbose);
    assert_eq!(config.show_progress, loaded_config.show_progress);
    assert_eq!(config.num_threads, loaded_config.num_threads);
}

#[test]
fn test_output_config_defaults() {
    let output_config = OutputConfig::default();

    assert_eq!(output_config.json_filename, "analysis_data.json");
    assert_eq!(output_config.csv_filename, "audio_quality_report.csv");
    assert!(output_config.include_timing);
    assert_eq!(output_config.min_quality_score, None);
    assert_eq!(output_config.output_dir, None);
}

#[test]
fn test_ffmpeg_config_defaults() {
    let ffmpeg_config = FfmpegConfig::default();

    assert_eq!(ffmpeg_config.log_level, "info");
    assert!(ffmpeg_config.hide_banner);
    assert_eq!(ffmpeg_config.timeout_seconds, Some(300));
}

#[test]
fn test_quality_thresholds_defaults() {
    let thresholds = QualityThresholds::default();

    // 验证关键阈值
    assert_eq!(thresholds.spectrum_fake_threshold, -85.0);
    assert_eq!(thresholds.spectrum_processed_threshold, -80.0);
    assert_eq!(thresholds.spectrum_good_threshold, -70.0);

    assert_eq!(thresholds.lra_poor_max, 3.0);
    assert_eq!(thresholds.lra_low_max, 6.0);
    assert_eq!(thresholds.lra_excellent_min, 8.0);
    assert_eq!(thresholds.lra_excellent_max, 12.0);

    assert_eq!(thresholds.peak_clipping_db, -0.1);
    assert_eq!(thresholds.peak_good_db, -6.0);
    assert_eq!(thresholds.peak_medium_db, -3.0);
}

#[test]
fn test_quality_thresholds_logical_order() {
    let thresholds = QualityThresholds::default();

    // 验证阈值的逻辑顺序
    assert!(thresholds.spectrum_fake_threshold < thresholds.spectrum_processed_threshold);
    assert!(thresholds.spectrum_processed_threshold < thresholds.spectrum_good_threshold);

    assert!(thresholds.lra_poor_max < thresholds.lra_low_max);
    assert!(thresholds.lra_low_max < thresholds.lra_excellent_min);
    assert!(thresholds.lra_excellent_min < thresholds.lra_excellent_max);
    assert!(thresholds.lra_excellent_max < thresholds.lra_acceptable_max);

    assert!(thresholds.peak_good_db < thresholds.peak_medium_db);
    assert!(thresholds.peak_medium_db < thresholds.peak_clipping_db);
}
