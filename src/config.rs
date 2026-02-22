//! # 配置管理模块
//!
//! 管理音频分析器的配置选项和参数设置。

use crate::error::{AnalyzerError, Result};
use crate::types::QualityThresholds;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 音频分析器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalyzerConfig {
    /// 支持的音频文件扩展名
    pub supported_extensions: Vec<String>,

    /// 质量评估阈值
    pub quality_thresholds: QualityThresholds,

    /// 并行处理线程数（None表示使用系统默认）
    pub num_threads: Option<usize>,

    /// 是否启用详细日志
    pub verbose: bool,

    /// 是否启用进度条
    pub show_progress: bool,

    /// 输出格式配置
    pub output: OutputConfig,

    /// FFmpeg 配置
    pub ffmpeg: FfmpegConfig,

    /// 扫描配置
    pub scan: ScanConfig,
}

/// 输出配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
    /// 输出目录
    pub output_dir: Option<PathBuf>,

    /// JSON 输出文件名
    pub json_filename: String,

    /// CSV 输出文件名
    pub csv_filename: String,

    /// 是否包含处理时间信息
    pub include_timing: bool,

    /// 最小质量分数过滤
    pub min_quality_score: Option<i32>,
}

/// FFmpeg 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FfmpegConfig {
    /// FFmpeg 日志级别
    pub log_level: String,

    /// 是否隐藏横幅信息
    pub hide_banner: bool,

    /// 超时时间（秒）
    pub timeout_seconds: Option<u64>,

    /// FFmpeg 最大并发进程数（None 表示自动）
    pub max_parallel_processes: Option<usize>,

    /// 单次命令可保留的 stderr 最大字节数
    pub stderr_max_bytes: usize,
}

/// 文件扫描配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanConfig {
    /// 是否跟随符号链接
    pub follow_links: bool,

    /// 扫描最大深度（None 表示不限制）
    pub max_depth: Option<usize>,

    /// 最多扫描到的音频文件数（None 表示不限制）
    pub max_files: Option<usize>,

    /// 是否限制在同一文件系统内扫描
    pub same_file_system: bool,

    /// 是否基于文件头进行魔数校验
    pub verify_magic_bytes: bool,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            supported_extensions: vec![
                "wav".to_string(),
                "mp3".to_string(),
                "m4a".to_string(),
                "flac".to_string(),
                "aac".to_string(),
                "ogg".to_string(),
                "opus".to_string(),
                "wma".to_string(),
                "aiff".to_string(),
                "alac".to_string(),
            ],
            quality_thresholds: QualityThresholds::default(),
            num_threads: None,
            verbose: false,
            show_progress: true,
            output: OutputConfig::default(),
            ffmpeg: FfmpegConfig::default(),
            scan: ScanConfig::default(),
        }
    }
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            output_dir: None,
            json_filename: "analysis_data.json".to_string(),
            csv_filename: "audio_quality_report.csv".to_string(),
            include_timing: true,
            min_quality_score: None,
        }
    }
}

impl Default for FfmpegConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            hide_banner: true,
            timeout_seconds: Some(300), // 5分钟超时
            max_parallel_processes: None,
            stderr_max_bytes: 2 * 1024 * 1024, // 2MB 上限，避免异常 stderr 撑爆内存
        }
    }
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            follow_links: false,
            max_depth: None,
            max_files: None,
            same_file_system: true,
            verify_magic_bytes: true,
        }
    }
}

impl AnalyzerConfig {
    /// 从文件加载配置
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: AnalyzerConfig = toml::from_str(&content)
            .map_err(|e| AnalyzerError::ConfigError(format!("配置文件解析错误: {e}")))?;
        Ok(config)
    }

    /// 保存配置到文件
    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| AnalyzerError::ConfigError(format!("配置序列化错误: {e}")))?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// 验证配置的有效性
    pub fn validate(&self) -> Result<()> {
        if self.supported_extensions.is_empty() {
            return Err(AnalyzerError::ConfigError(
                "支持的文件扩展名列表不能为空".to_string(),
            ));
        }

        if let Some(threads) = self.num_threads {
            if threads == 0 {
                return Err(AnalyzerError::ConfigError("线程数必须大于0".to_string()));
            }
        }

        if let Some(timeout) = self.ffmpeg.timeout_seconds {
            if timeout == 0 {
                return Err(AnalyzerError::ConfigError(
                    "FFmpeg 超时时间必须大于0".to_string(),
                ));
            }
        }

        if let Some(max_parallel) = self.ffmpeg.max_parallel_processes {
            if max_parallel == 0 {
                return Err(AnalyzerError::ConfigError(
                    "FFmpeg 最大并发进程数必须大于0".to_string(),
                ));
            }
        }

        if self.ffmpeg.stderr_max_bytes == 0 {
            return Err(AnalyzerError::ConfigError(
                "FFmpeg stderr 缓冲上限必须大于0".to_string(),
            ));
        }

        if let Some(depth) = self.scan.max_depth {
            if depth == 0 {
                return Err(AnalyzerError::ConfigError(
                    "扫描最大深度必须大于0".to_string(),
                ));
            }
        }

        if let Some(max_files) = self.scan.max_files {
            if max_files == 0 {
                return Err(AnalyzerError::ConfigError(
                    "扫描最大文件数必须大于0".to_string(),
                ));
            }
        }

        // 验证质量阈值的合理性
        let thresholds = &self.quality_thresholds;
        if thresholds.lra_poor_max >= thresholds.lra_low_max {
            return Err(AnalyzerError::ConfigError(
                "LRA阈值配置不合理: poor_max 应小于 low_max".to_string(),
            ));
        }

        if thresholds.peak_good_db >= thresholds.peak_medium_db {
            return Err(AnalyzerError::ConfigError(
                "峰值阈值配置不合理: good_db 应小于 medium_db".to_string(),
            ));
        }

        Ok(())
    }

    /// 检查文件扩展名是否支持
    pub fn is_supported_extension(&self, extension: &str) -> bool {
        self.supported_extensions
            .iter()
            .any(|ext| ext.eq_ignore_ascii_case(extension))
    }

    /// 获取有效的线程数
    pub fn effective_thread_count(&self) -> usize {
        self.num_threads.unwrap_or_else(num_cpus::get)
    }
}

/// 从环境变量或默认值创建配置
pub fn create_default_config() -> AnalyzerConfig {
    let mut config = AnalyzerConfig::default();

    // 从环境变量读取一些配置
    if let Ok(threads) = std::env::var("AUDIO_ANALYZER_THREADS") {
        if let Ok(num) = threads.parse::<usize>() {
            config.num_threads = Some(num);
        }
    }

    if let Ok(verbose) = std::env::var("AUDIO_ANALYZER_VERBOSE") {
        config.verbose = verbose.eq_ignore_ascii_case("true") || verbose == "1";
    }

    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = AnalyzerConfig::default();
        assert!(!config.supported_extensions.is_empty());
        assert!(config.is_supported_extension("wav"));
        assert!(config.is_supported_extension("MP3"));
        assert!(!config.is_supported_extension("txt"));
    }

    #[test]
    fn test_config_validation() {
        let config = AnalyzerConfig::default();
        assert!(config.validate().is_ok());

        let mut invalid_config = config.clone();
        invalid_config.supported_extensions.clear();
        assert!(invalid_config.validate().is_err());
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
    }

    #[test]
    fn test_effective_thread_count() {
        let mut config = AnalyzerConfig::default();

        // 默认情况下应该使用系统CPU数量
        assert!(config.effective_thread_count() > 0);

        // 设置特定线程数
        config.num_threads = Some(4);
        assert_eq!(config.effective_thread_count(), 4);
    }
}
