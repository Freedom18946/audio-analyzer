//! # 错误处理模块
//!
//! 定义了音频分析器中使用的所有错误类型和错误处理机制。

use std::fmt;

/// 音频分析器的结果类型
pub type Result<T> = std::result::Result<T, AnalyzerError>;

/// 音频分析器错误类型
#[derive(Debug)]
pub enum AnalyzerError {
    /// I/O 错误
    Io(std::io::Error),

    /// FFmpeg 执行错误
    FfmpegError {
        /// 错误消息
        message: String,
        /// 命令输出
        stderr: Option<String>,
    },

    /// 文件格式不支持
    UnsupportedFormat {
        /// 文件路径
        path: String,
        /// 文件扩展名
        extension: Option<String>,
    },

    /// 数据解析错误
    ParseError {
        /// 错误描述
        message: String,
        /// 原始数据
        raw_data: Option<String>,
    },

    /// 配置错误
    ConfigError(String),

    /// 依赖项设置错误
    DependencyError(String),

    /// 其他错误
    Other(String),
}

impl fmt::Display for AnalyzerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalyzerError::Io(err) => write!(f, "I/O 错误: {err}"),
            AnalyzerError::FfmpegError { message, stderr } => {
                write!(f, "FFmpeg 执行错误: {message}")?;
                if let Some(stderr) = stderr {
                    write!(f, "\n详细信息: {stderr}")?;
                }
                Ok(())
            }
            AnalyzerError::UnsupportedFormat { path, extension } => {
                write!(f, "不支持的文件格式: {path}")?;
                if let Some(ext) = extension {
                    write!(f, " (扩展名: {ext})")?;
                }
                Ok(())
            }
            AnalyzerError::ParseError { message, raw_data } => {
                write!(f, "数据解析错误: {message}")?;
                if let Some(data) = raw_data {
                    write!(f, "\n原始数据: {data}")?;
                }
                Ok(())
            }
            AnalyzerError::ConfigError(msg) => write!(f, "配置错误: {msg}"),
            AnalyzerError::DependencyError(msg) => write!(f, "依赖项错误: {msg}"),
            AnalyzerError::Other(msg) => write!(f, "错误: {msg}"),
        }
    }
}

impl std::error::Error for AnalyzerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AnalyzerError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for AnalyzerError {
    fn from(err: std::io::Error) -> Self {
        AnalyzerError::Io(err)
    }
}

impl From<serde_json::Error> for AnalyzerError {
    fn from(err: serde_json::Error) -> Self {
        AnalyzerError::ParseError {
            message: format!("JSON 解析错误: {err}"),
            raw_data: None,
        }
    }
}

/// 便捷的错误创建宏
#[macro_export]
macro_rules! analyzer_error {
    ($kind:ident, $msg:expr) => {
        AnalyzerError::$kind($msg.to_string())
    };
    ($kind:ident, $msg:expr, $($arg:tt)*) => {
        AnalyzerError::$kind(format!($msg, $($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AnalyzerError::ConfigError("测试错误".to_string());
        assert!(err.to_string().contains("配置错误"));
        assert!(err.to_string().contains("测试错误"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "文件未找到");
        let analyzer_err: AnalyzerError = io_err.into();

        match analyzer_err {
            AnalyzerError::Io(_) => (),
            _ => panic!("应该转换为 Io 错误"),
        }
    }
}
