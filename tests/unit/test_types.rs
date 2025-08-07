//! # 数据类型模块单元测试
//!
//! 测试音频分析相关数据结构的功能

use audio_analyzer_ultimate::types::{AnalysisProgress, AudioMetrics, AudioStats};

#[test]
fn test_audio_metrics_creation() {
    let metrics = AudioMetrics::new("test.wav".to_string(), 1024);

    assert_eq!(metrics.file_path, "test.wav");
    assert_eq!(metrics.file_size_bytes, 1024);
    assert_eq!(metrics.lra, None);
    assert_eq!(metrics.peak_amplitude_db, None);
    assert_eq!(metrics.overall_rms_db, None);
    assert_eq!(metrics.rms_db_above_16k, None);
    assert_eq!(metrics.rms_db_above_18k, None);
    assert_eq!(metrics.rms_db_above_20k, None);
    assert_eq!(metrics.processing_time_ms, 0);
}

#[test]
fn test_audio_metrics_completeness() {
    let mut metrics = AudioMetrics::new("test.wav".to_string(), 1024);

    // 初始状态应该是不完整的
    assert!(!metrics.is_complete());

    // 设置部分数据
    metrics.lra = Some(10.0);
    assert!(!metrics.is_complete());

    metrics.peak_amplitude_db = Some(-6.0);
    assert!(!metrics.is_complete());

    // 设置所有必需数据
    metrics.rms_db_above_18k = Some(-70.0);
    assert!(metrics.is_complete());
}

#[test]
fn test_audio_metrics_filename() {
    let test_cases = vec![
        ("test.wav", "test.wav"),
        ("/path/to/test.wav", "test.wav"),
        ("/very/long/path/to/audio/file.flac", "file.flac"),
        ("", "未知文件"),
    ];

    // Windows路径测试（仅在Windows上测试）
    #[cfg(windows)]
    let windows_cases = vec![("C:\\Windows\\test.mp3", "test.mp3")];

    #[cfg(windows)]
    for (input_path, expected_filename) in windows_cases {
        let metrics = AudioMetrics::new(input_path.to_string(), 1024);
        assert_eq!(metrics.filename(), expected_filename);
    }

    for (input_path, expected_filename) in test_cases {
        let metrics = AudioMetrics::new(input_path.to_string(), 1024);
        assert_eq!(metrics.filename(), expected_filename);
    }
}

#[test]
fn test_audio_metrics_serialization() {
    let mut metrics = AudioMetrics::new("test.wav".to_string(), 1024);
    metrics.lra = Some(8.5);
    metrics.peak_amplitude_db = Some(-3.2);
    metrics.overall_rms_db = Some(-18.7);
    metrics.rms_db_above_16k = Some(-65.3);
    metrics.rms_db_above_18k = Some(-72.1);
    metrics.rms_db_above_20k = Some(-85.4);
    metrics.processing_time_ms = 1500;

    // 序列化为JSON
    let json = serde_json::to_string(&metrics).unwrap();
    assert!(json.contains("filePath"));
    assert!(json.contains("fileSizeBytes"));
    assert!(json.contains("lra"));
    assert!(json.contains("peakAmplitudeDb"));
    assert!(json.contains("processingTimeMs"));

    // 反序列化
    let deserialized: AudioMetrics = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.file_path, metrics.file_path);
    assert_eq!(deserialized.file_size_bytes, metrics.file_size_bytes);
    assert_eq!(deserialized.lra, metrics.lra);
    assert_eq!(deserialized.peak_amplitude_db, metrics.peak_amplitude_db);
    assert_eq!(deserialized.processing_time_ms, metrics.processing_time_ms);
}

#[test]
fn test_audio_stats_creation() {
    let stats = AudioStats::new();

    assert_eq!(stats.peak_db, None);
    assert_eq!(stats.rms_db, None);
    assert!(!stats.has_data());
}

#[test]
fn test_audio_stats_has_data() {
    let mut stats = AudioStats::new();

    // 初始状态没有数据
    assert!(!stats.has_data());

    // 设置峰值数据
    stats.peak_db = Some(-6.0);
    assert!(stats.has_data());

    // 清除峰值，设置RMS
    stats.peak_db = None;
    stats.rms_db = Some(-18.0);
    assert!(stats.has_data());

    // 设置两个值
    stats.peak_db = Some(-3.0);
    assert!(stats.has_data());

    // 清除所有值
    stats.peak_db = None;
    stats.rms_db = None;
    assert!(!stats.has_data());
}

#[test]
fn test_audio_stats_default() {
    let stats = AudioStats::default();

    assert_eq!(stats.peak_db, None);
    assert_eq!(stats.rms_db, None);
    assert!(!stats.has_data());
}

#[test]
fn test_analysis_progress_percentage() {
    let test_cases = vec![
        (0, 10, 0.0),
        (5, 10, 50.0),
        (10, 10, 100.0),
        (3, 7, 42.857142857142854),
        (0, 0, 0.0), // 边界情况：总数为0
    ];

    for (completed, total, expected_percentage) in test_cases {
        let progress = AnalysisProgress {
            current_file: completed,
            total_files: total,
            current_path: "test.wav".to_string(),
            completed_files: completed,
        };

        let percentage = progress.percentage();
        if total == 0 {
            assert_eq!(percentage, 0.0);
        } else {
            assert!((percentage - expected_percentage).abs() < 0.0001);
        }
    }
}

#[test]
fn test_analysis_progress_fields() {
    let progress = AnalysisProgress {
        current_file: 5,
        total_files: 10,
        current_path: "/path/to/current.wav".to_string(),
        completed_files: 4,
    };

    assert_eq!(progress.current_file, 5);
    assert_eq!(progress.total_files, 10);
    assert_eq!(progress.current_path, "/path/to/current.wav");
    assert_eq!(progress.completed_files, 4);
    assert_eq!(progress.percentage(), 40.0);
}
