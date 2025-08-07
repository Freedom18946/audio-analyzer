//! # 工具函数模块单元测试
//!
//! 测试各种工具函数的正确性

use audio_analyzer_ultimate::utils::{fs_utils, string_utils, Timer};
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn test_is_supported_audio_file() {
    let extensions = vec!["wav".to_string(), "mp3".to_string(), "flac".to_string()];

    // 测试支持的格式
    assert!(fs_utils::is_supported_audio_file(
        Path::new("test.wav"),
        &extensions
    ));
    assert!(fs_utils::is_supported_audio_file(
        Path::new("test.WAV"),
        &extensions
    ));
    assert!(fs_utils::is_supported_audio_file(
        Path::new("test.Mp3"),
        &extensions
    ));
    assert!(fs_utils::is_supported_audio_file(
        Path::new("test.FLAC"),
        &extensions
    ));
    assert!(fs_utils::is_supported_audio_file(
        Path::new("/path/to/music.wav"),
        &extensions
    ));

    // 测试不支持的格式
    assert!(!fs_utils::is_supported_audio_file(
        Path::new("test.txt"),
        &extensions
    ));
    assert!(!fs_utils::is_supported_audio_file(
        Path::new("test.doc"),
        &extensions
    ));
    assert!(!fs_utils::is_supported_audio_file(
        Path::new("test"),
        &extensions
    ));
    assert!(!fs_utils::is_supported_audio_file(
        Path::new("test."),
        &extensions
    ));
}

#[test]
fn test_get_display_name() {
    let test_cases = vec![
        ("test.wav", "test.wav"),
        ("/path/to/test.wav", "test.wav"),
        ("/very/long/path/to/audio/file.flac", "file.flac"),
        ("", "未知文件"),
        ("no_extension", "no_extension"),
    ];

    // Windows路径测试（仅在Windows上测试）
    #[cfg(windows)]
    let windows_cases = vec![("C:\\Windows\\test.mp3", "test.mp3")];

    #[cfg(windows)]
    for (input_path, expected_name) in windows_cases {
        let result = fs_utils::get_display_name(input_path);
        assert_eq!(result, expected_name);
    }

    for (input_path, expected_name) in test_cases {
        let result = fs_utils::get_display_name(input_path);
        assert_eq!(result, expected_name);
    }
}

#[test]
fn test_ensure_dir_exists() {
    let temp_dir = TempDir::new().unwrap();
    let test_dir = temp_dir.path().join("test_subdir");

    // 目录不存在
    assert!(!test_dir.exists());

    // 创建目录
    fs_utils::ensure_dir_exists(&test_dir).unwrap();
    assert!(test_dir.exists());
    assert!(test_dir.is_dir());

    // 再次调用应该成功（目录已存在）
    fs_utils::ensure_dir_exists(&test_dir).unwrap();
    assert!(test_dir.exists());
}

#[test]
fn test_format_file_size() {
    let test_cases = vec![
        (0, "0 B"),
        (512, "512 B"),
        (1024, "1.0 KB"),
        (1536, "1.5 KB"),
        (1048576, "1.0 MB"),
        (1073741824, "1.0 GB"),
        (1099511627776, "1.0 TB"),
        (2048, "2.0 KB"),
        (1536000, "1.5 MB"),
    ];

    for (bytes, expected) in test_cases {
        let result = string_utils::format_file_size(bytes);
        assert_eq!(result, expected);
    }
}

#[test]
fn test_truncate_string() {
    let test_cases = vec![
        ("hello", 10, "hello"),
        ("hello world", 8, "hello..."),
        ("hi", 5, "hi"),
        (
            "a very long string that needs truncation",
            15,
            "a very long ...",
        ),
        ("", 5, ""),
        ("abc", 3, "abc"),
        ("abcd", 3, "..."),
    ];

    for (input, max_len, expected) in test_cases {
        let result = string_utils::truncate_string(input, max_len);
        assert_eq!(result, expected);
    }
}

#[test]
fn test_format_duration() {
    let test_cases = vec![
        (Duration::from_millis(500), "500ms"),
        (Duration::from_secs(1), "1.000s"),
        (Duration::from_secs(30), "30.000s"),
        (Duration::from_secs(60), "1m 0s"),
        (Duration::from_secs(90), "1m 30s"),
        (Duration::from_secs(3600), "1h 0m 0s"),
        (Duration::from_secs(3661), "1h 1m 1s"),
        (Duration::from_millis(1500), "1.500s"),
    ];

    for (duration, expected) in test_cases {
        let result = string_utils::format_duration(duration);
        assert_eq!(result, expected);
    }
}

#[test]
fn test_timer_basic_functionality() {
    let timer = Timer::new("test");

    // 等待一小段时间
    std::thread::sleep(Duration::from_millis(10));

    let elapsed = timer.elapsed();
    assert!(elapsed.as_millis() >= 10);
    assert!(elapsed.as_millis() < 100); // 应该不会太长
}

#[test]
fn test_timer_reset() {
    let mut timer = Timer::new("test");

    // 等待一段时间
    std::thread::sleep(Duration::from_millis(10));
    let first_elapsed = timer.elapsed();

    // 重置计时器
    timer.reset();

    // 立即检查，应该接近0
    let second_elapsed = timer.elapsed();
    assert!(second_elapsed < first_elapsed);
    assert!(second_elapsed.as_millis() < 5);
}

#[test]
fn test_timer_stop() {
    let timer = Timer::new("test");

    std::thread::sleep(Duration::from_millis(10));

    let duration = timer.stop();
    assert!(duration.as_millis() >= 10);
}

#[test]
fn test_scan_audio_files() {
    let temp_dir = TempDir::new().unwrap();
    let extensions = vec!["wav".to_string(), "mp3".to_string()];

    // 创建测试文件
    let audio_file1 = temp_dir.path().join("test1.wav");
    let audio_file2 = temp_dir.path().join("test2.mp3");
    let text_file = temp_dir.path().join("readme.txt");
    let subdir = temp_dir.path().join("subdir");

    std::fs::create_dir(&subdir).unwrap();
    let audio_file3 = subdir.join("test3.wav");

    // 创建文件
    std::fs::write(&audio_file1, "fake wav content").unwrap();
    std::fs::write(&audio_file2, "fake mp3 content").unwrap();
    std::fs::write(&text_file, "text content").unwrap();
    std::fs::write(&audio_file3, "fake wav content").unwrap();

    // 扫描音频文件
    let found_files = fs_utils::scan_audio_files(temp_dir.path(), &extensions).unwrap();

    // 应该找到3个音频文件
    assert_eq!(found_files.len(), 3);

    // 验证找到的文件
    let file_names: Vec<String> = found_files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    assert!(file_names.contains(&"test1.wav".to_string()));
    assert!(file_names.contains(&"test2.mp3".to_string()));
    assert!(file_names.contains(&"test3.wav".to_string()));
}

#[test]
fn test_get_file_size() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");

    let content = "Hello, World!";
    std::fs::write(&test_file, content).unwrap();

    let size = fs_utils::get_file_size(&test_file).unwrap();
    assert_eq!(size, content.len() as u64);
}
