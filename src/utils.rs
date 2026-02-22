//! # 通用工具函数模块
//!
//! 提供音频分析器中使用的各种通用工具函数。

use crate::config::ScanConfig;
use crate::error::{AnalyzerError, Result};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// 文件系统相关工具
pub mod fs_utils {
    use super::*;
    use walkdir::WalkDir;

    /// 递归扫描目录，查找支持的音频文件
    pub fn scan_audio_files<P: AsRef<Path>>(
        dir: P,
        supported_extensions: &[String],
        scan_config: &ScanConfig,
    ) -> Result<Vec<PathBuf>> {
        let mut audio_files = Vec::new();
        let mut walker = WalkDir::new(dir)
            .follow_links(scan_config.follow_links)
            .same_file_system(scan_config.same_file_system);

        if let Some(max_depth) = scan_config.max_depth {
            walker = walker.max_depth(max_depth);
        }

        for entry in walker {
            let entry = entry.map_err(|e| AnalyzerError::Io(e.into()))?;

            if entry.file_type().is_file() {
                let path = entry.path();
                if is_supported_audio_file(path, supported_extensions) {
                    if scan_config.verify_magic_bytes && !has_expected_audio_magic(path)? {
                        continue;
                    }
                    audio_files.push(path.to_path_buf());
                    if let Some(max_files) = scan_config.max_files {
                        if audio_files.len() >= max_files {
                            break;
                        }
                    }
                }
            }
        }

        Ok(audio_files)
    }

    /// 检查文件是否为支持的音频格式
    pub fn is_supported_audio_file(path: &Path, supported_extensions: &[String]) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| {
                supported_extensions
                    .iter()
                    .any(|supported| supported.eq_ignore_ascii_case(ext))
            })
            .unwrap_or(false)
    }

    /// 获取文件大小
    pub fn get_file_size<P: AsRef<Path>>(path: P) -> Result<u64> {
        let metadata = fs::metadata(path)?;
        Ok(metadata.len())
    }

    /// 确保目录存在，如果不存在则创建
    pub fn ensure_dir_exists<P: AsRef<Path>>(dir: P) -> Result<()> {
        let path = dir.as_ref();
        if !path.exists() {
            fs::create_dir_all(path)?;
        }
        Ok(())
    }

    /// 获取文件的显示名称（不含路径）
    pub fn get_display_name<P: AsRef<Path>>(path: P) -> String {
        path.as_ref()
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("未知文件")
            .to_string()
    }

    fn has_expected_audio_magic(path: &Path) -> Result<bool> {
        let mut file = std::fs::File::open(path)?;
        let mut buf = [0u8; 64];
        let read_len = file.read(&mut buf)?;
        if read_len == 0 {
            return Ok(false);
        }
        let header = &buf[..read_len];

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();

        let is_match = match ext.as_str() {
            "wav" => header.len() >= 12 && &header[0..4] == b"RIFF" && &header[8..12] == b"WAVE",
            "flac" => header.starts_with(b"fLaC"),
            "mp3" => {
                header.starts_with(b"ID3")
                    || (header.len() >= 2 && header[0] == 0xFF && (header[1] & 0xE0) == 0xE0)
            }
            "m4a" | "alac" => header.len() >= 8 && &header[4..8] == b"ftyp",
            "aac" => {
                (header.len() >= 2 && header[0] == 0xFF && (header[1] & 0xF0) == 0xF0)
                    || (header.len() >= 8 && &header[4..8] == b"ftyp")
            }
            "ogg" | "opus" => header.starts_with(b"OggS"),
            "wma" => header.starts_with(&[
                0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62,
                0xCE, 0x6C,
            ]),
            "aiff" => {
                header.len() >= 12
                    && &header[0..4] == b"FORM"
                    && (&header[8..12] == b"AIFF" || &header[8..12] == b"AIFC")
            }
            _ => true,
        };

        Ok(is_match)
    }
}

/// 进程执行相关工具
pub mod process_utils {
    use super::*;

    /// 执行命令并获取stderr输出
    pub fn run_command_capture_stderr(
        mut command: Command,
        timeout: Option<Duration>,
        stderr_max_bytes: usize,
    ) -> Result<String> {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let mut child = command.spawn()?;
        let stderr_pipe = child.stderr.take().ok_or_else(|| {
            AnalyzerError::Other("无法捕获子进程 stderr：stderr 管道不可用".to_string())
        })?;

        let reader = thread::spawn(move || read_stderr_limited(stderr_pipe, stderr_max_bytes));
        let start = Instant::now();

        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }

            if let Some(limit) = timeout {
                if start.elapsed() >= limit {
                    let _ = child.kill();
                    let _ = child.wait();
                    let (stderr_bytes, truncated, dropped_bytes) =
                        reader.join().unwrap_or_else(|_| (Vec::new(), false, 0));
                    let stderr = format_stderr(stderr_bytes, truncated, dropped_bytes);
                    return Err(AnalyzerError::TimeoutError {
                        message: format!("命令执行超时（{} 秒）", limit.as_secs()),
                        stderr: if stderr.is_empty() {
                            None
                        } else {
                            Some(stderr)
                        },
                    });
                }
            }

            thread::sleep(Duration::from_millis(20));
        };

        let (stderr_bytes, truncated, dropped_bytes) =
            reader.join().unwrap_or_else(|_| (Vec::new(), false, 0));
        let stderr = format_stderr(stderr_bytes, truncated, dropped_bytes);

        if !status.success() {
            return Err(AnalyzerError::FfmpegError {
                message: format!("命令执行失败，退出码: {:?}", status.code()),
                stderr: if stderr.is_empty() {
                    None
                } else {
                    Some(stderr)
                },
            });
        }

        Ok(stderr)
    }

    /// 检查命令是否执行成功
    pub fn check_command_success(mut command: Command) -> Result<bool> {
        let status = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        Ok(status.success())
    }

    fn read_stderr_limited<R: Read>(mut reader: R, limit: usize) -> (Vec<u8>, bool, usize) {
        let mut stderr = Vec::new();
        let mut buf = [0u8; 8192];
        let mut dropped_bytes = 0usize;
        let mut truncated = false;

        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if stderr.len() < limit {
                        let remaining = limit - stderr.len();
                        let to_copy = remaining.min(n);
                        stderr.extend_from_slice(&buf[..to_copy]);
                        if to_copy < n {
                            truncated = true;
                            dropped_bytes += n - to_copy;
                        }
                    } else {
                        truncated = true;
                        dropped_bytes += n;
                    }
                }
                Err(_) => break,
            }
        }

        (stderr, truncated, dropped_bytes)
    }

    fn format_stderr(stderr_bytes: Vec<u8>, truncated: bool, dropped_bytes: usize) -> String {
        let mut stderr = String::from_utf8_lossy(&stderr_bytes).to_string();
        if truncated {
            stderr.push_str(&format!(
                "\n[stderr truncated, omitted {} bytes]",
                dropped_bytes
            ));
        }
        stderr
    }
}

/// 字符串处理工具
pub mod string_utils {
    /// 截断字符串到指定长度，如果超长则添加省略号
    pub fn truncate_string(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            format!("{}...", &s[..max_len.saturating_sub(3)])
        }
    }

    /// 格式化文件大小为人类可读的格式
    pub fn format_file_size(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        const THRESHOLD: f64 = 1024.0;

        if bytes == 0 {
            return "0 B".to_string();
        }

        let mut size = bytes as f64;
        let mut unit_index = 0;

        while size >= THRESHOLD && unit_index < UNITS.len() - 1 {
            size /= THRESHOLD;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{bytes} {}", UNITS[unit_index])
        } else {
            format!("{size:.1} {}", UNITS[unit_index])
        }
    }

    /// 格式化持续时间为人类可读的格式
    pub fn format_duration(duration: std::time::Duration) -> String {
        let total_seconds = duration.as_secs();
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;
        let millis = duration.subsec_millis();

        if hours > 0 {
            format!("{hours}h {minutes}m {seconds}s")
        } else if minutes > 0 {
            format!("{minutes}m {seconds}s")
        } else if seconds > 0 {
            format!("{seconds}.{millis:03}s")
        } else {
            format!("{millis}ms")
        }
    }
}

/// 性能测量工具
pub struct Timer {
    start: Instant,
    name: String,
}

impl Timer {
    /// 创建新的计时器
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            start: Instant::now(),
            name: name.into(),
        }
    }

    /// 获取已经过的时间
    pub fn elapsed(&self) -> std::time::Duration {
        self.start.elapsed()
    }

    /// 重置计时器
    pub fn reset(&mut self) {
        self.start = Instant::now();
    }

    /// 停止计时器并返回持续时间
    pub fn stop(self) -> std::time::Duration {
        self.elapsed()
    }

    /// 打印经过的时间
    pub fn print_elapsed(&self) {
        println!(
            "{}: {}",
            self.name,
            string_utils::format_duration(self.elapsed())
        );
    }
}

/// 用户输入工具
pub mod input_utils {
    use super::*;
    use std::io::{self, Write};

    /// 从用户获取文件夹路径
    pub fn get_folder_path_from_user() -> Result<PathBuf> {
        loop {
            print!("请输入要递归处理的音乐顶层文件夹路径: ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            let path = PathBuf::from(input.trim());

            if path.is_dir() {
                return Ok(path.canonicalize()?);
            } else {
                eprintln!(
                    "错误: \"{}\" 不是一个有效的文件夹路径或不存在，请重新输入。",
                    path.display()
                );
            }
        }
    }

    /// 询问用户是否继续
    pub fn ask_user_confirmation(message: &str) -> Result<bool> {
        print!("{message} (y/N): ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let response = input.trim().to_lowercase();
        Ok(response == "y" || response == "yes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ScanConfig;
    use tempfile::TempDir;

    #[test]
    fn test_format_file_size() {
        assert_eq!(string_utils::format_file_size(0), "0 B");
        assert_eq!(string_utils::format_file_size(512), "512 B");
        assert_eq!(string_utils::format_file_size(1024), "1.0 KB");
        assert_eq!(string_utils::format_file_size(1536), "1.5 KB");
        assert_eq!(string_utils::format_file_size(1048576), "1.0 MB");
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(string_utils::truncate_string("hello", 10), "hello");
        assert_eq!(string_utils::truncate_string("hello world", 8), "hello...");
        assert_eq!(string_utils::truncate_string("hi", 5), "hi");
    }

    #[test]
    fn test_is_supported_audio_file() {
        let extensions = vec!["wav".to_string(), "mp3".to_string()];

        assert!(fs_utils::is_supported_audio_file(
            Path::new("test.wav"),
            &extensions
        ));
        assert!(fs_utils::is_supported_audio_file(
            Path::new("test.MP3"),
            &extensions
        ));
        assert!(!fs_utils::is_supported_audio_file(
            Path::new("test.txt"),
            &extensions
        ));
    }

    #[test]
    fn test_timer() {
        let timer = Timer::new("test");
        std::thread::sleep(std::time::Duration::from_millis(10));
        let elapsed = timer.elapsed();
        assert!(elapsed.as_millis() >= 10);
    }

    #[test]
    fn test_ensure_dir_exists() {
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("test_subdir");

        assert!(!test_dir.exists());
        fs_utils::ensure_dir_exists(&test_dir).unwrap();
        assert!(test_dir.exists());
    }

    #[test]
    fn test_scan_audio_files_respects_limits_and_magic() {
        let temp_dir = TempDir::new().unwrap();
        let extensions = vec!["wav".to_string()];
        let good_wav = temp_dir.path().join("good.wav");
        let fake_wav = temp_dir.path().join("fake.wav");
        let nested_dir = temp_dir.path().join("nested");
        let nested_wav = nested_dir.join("nested.wav");
        std::fs::create_dir(&nested_dir).unwrap();

        // 最小 WAV 头
        std::fs::write(&good_wav, b"RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00").unwrap();
        std::fs::write(&fake_wav, b"not-audio").unwrap();
        std::fs::write(&nested_wav, b"RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00").unwrap();

        let mut config = ScanConfig {
            max_depth: Some(1),
            ..ScanConfig::default()
        };
        let files = fs_utils::scan_audio_files(temp_dir.path(), &extensions, &config).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files.iter().any(|p| p.ends_with("good.wav")));

        config.max_depth = None;
        config.max_files = Some(1);
        let files = fs_utils::scan_audio_files(temp_dir.path(), &extensions, &config).unwrap();
        assert_eq!(files.len(), 1);
    }
}
