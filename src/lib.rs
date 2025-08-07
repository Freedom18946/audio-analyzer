//! # 音频质量分析器库
//!
//! 这是一个高性能的音频质量分析工具库，提供音频文件的质量评估和分析功能。
//!
//! ## 主要功能
//!
//! - **音频质量分析**: 使用FFmpeg进行音频数据提取和分析
//! - **并行处理**: 利用Rayon进行高效的并行文件处理
//! - **多格式支持**: 支持WAV、MP3、FLAC、AAC等多种音频格式
//! - **详细报告**: 生成包含LRA、峰值、RMS等指标的详细分析报告
//!
//! ## 架构设计
//!
//! 本库采用模块化设计，主要包含以下模块：
//!
//! - `analyzer`: 核心音频分析功能
//! - `config`: 配置管理
//! - `utils`: 通用工具函数
//! - `error`: 错误处理
//! - `types`: 数据类型定义

pub mod analyzer;
pub mod config;
pub mod error;
pub mod types;
pub mod utils;

// 重新导出主要的公共API
pub use analyzer::AudioAnalyzer;
pub use config::AnalyzerConfig;
pub use error::{AnalyzerError, Result};
pub use types::{AudioMetrics, QualityThresholds};

/// 库版本信息
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 支持的音频文件扩展名
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "wav", "mp3", "m4a", "flac", "aac", "ogg", "opus", "wma", "aiff", "alac",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_exists() {
        assert!(
            VERSION.starts_with(char::is_numeric),
            "版本字符串应该以数字开头"
        );
    }

    #[test]
    fn test_supported_extensions() {
        assert!(SUPPORTED_EXTENSIONS.contains(&"wav"));
        assert!(SUPPORTED_EXTENSIONS.contains(&"mp3"));
        assert!(SUPPORTED_EXTENSIONS.contains(&"flac"));
    }
}
