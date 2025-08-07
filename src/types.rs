//! # 数据类型定义模块
//!
//! 定义了音频分析器中使用的所有数据结构和类型。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 音频文件的分析指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioMetrics {
    /// 文件路径
    #[serde(rename = "filePath")]
    pub file_path: String,

    /// 文件大小（字节）
    #[serde(rename = "fileSizeBytes")]
    pub file_size_bytes: u64,

    /// 响度范围 (Loudness Range) - EBU R128 标准
    #[serde(rename = "lra")]
    pub lra: Option<f64>,

    /// 峰值振幅 (dB)
    #[serde(rename = "peakAmplitudeDb")]
    pub peak_amplitude_db: Option<f64>,

    /// 整体RMS电平 (dB)
    #[serde(rename = "overallRmsDb")]
    pub overall_rms_db: Option<f64>,

    /// 16kHz以上频段的RMS电平 (dB)
    #[serde(rename = "rmsDbAbove16k")]
    pub rms_db_above_16k: Option<f64>,

    /// 18kHz以上频段的RMS电平 (dB)
    #[serde(rename = "rmsDbAbove18k")]
    pub rms_db_above_18k: Option<f64>,

    /// 20kHz以上频段的RMS电平 (dB)
    #[serde(rename = "rmsDbAbove20k")]
    pub rms_db_above_20k: Option<f64>,

    /// 处理时间（毫秒）
    #[serde(rename = "processingTimeMs")]
    pub processing_time_ms: u64,
}

impl AudioMetrics {
    /// 创建新的音频指标实例
    pub fn new(file_path: String, file_size_bytes: u64) -> Self {
        Self {
            file_path,
            file_size_bytes,
            lra: None,
            peak_amplitude_db: None,
            overall_rms_db: None,
            rms_db_above_16k: None,
            rms_db_above_18k: None,
            rms_db_above_20k: None,
            processing_time_ms: 0,
        }
    }

    /// 检查数据完整性
    pub fn is_complete(&self) -> bool {
        self.lra.is_some() && self.peak_amplitude_db.is_some() && self.rms_db_above_18k.is_some()
    }

    /// 获取文件名（不含路径）
    pub fn filename(&self) -> String {
        PathBuf::from(&self.file_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("未知文件")
            .to_string()
    }
}

/// 音频统计信息（用于FFmpeg astats输出解析）
#[derive(Debug, Clone)]
pub struct AudioStats {
    /// 峰值电平 (dB)
    pub peak_db: Option<f64>,
    /// RMS电平 (dB)
    pub rms_db: Option<f64>,
}

impl AudioStats {
    /// 创建新的音频统计实例
    pub fn new() -> Self {
        Self {
            peak_db: None,
            rms_db: None,
        }
    }

    /// 检查是否有有效数据
    pub fn has_data(&self) -> bool {
        self.peak_db.is_some() || self.rms_db.is_some()
    }
}

impl Default for AudioStats {
    fn default() -> Self {
        Self::new()
    }
}

/// 质量评估阈值配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityThresholds {
    /// 频谱伪造检测阈值 (dB)
    pub spectrum_fake_threshold: f64,
    /// 频谱处理检测阈值 (dB)
    pub spectrum_processed_threshold: f64,
    /// 频谱良好阈值 (dB)
    pub spectrum_good_threshold: f64,

    /// LRA 差劲最大值 (LU)
    pub lra_poor_max: f64,
    /// LRA 低动态最大值 (LU)
    pub lra_low_max: f64,
    /// LRA 优秀最小值 (LU)
    pub lra_excellent_min: f64,
    /// LRA 优秀最大值 (LU)
    pub lra_excellent_max: f64,
    /// LRA 可接受最大值 (LU)
    pub lra_acceptable_max: f64,
    /// LRA 过高阈值 (LU)
    pub lra_too_high: f64,

    /// 峰值削波检测阈值 (dB)
    pub peak_clipping_db: f64,
    /// 峰值削波检测阈值（线性）
    pub peak_clipping_linear: f64,
    /// 峰值良好阈值 (dB)
    pub peak_good_db: f64,
    /// 峰值中等阈值 (dB)
    pub peak_medium_db: f64,
}

impl Default for QualityThresholds {
    fn default() -> Self {
        Self {
            spectrum_fake_threshold: -85.0,
            spectrum_processed_threshold: -80.0,
            spectrum_good_threshold: -70.0,
            lra_poor_max: 3.0,
            lra_low_max: 6.0,
            lra_excellent_min: 8.0,
            lra_excellent_max: 12.0,
            lra_acceptable_max: 15.0,
            lra_too_high: 20.0,
            peak_clipping_db: -0.1,
            peak_clipping_linear: 0.999,
            peak_good_db: -6.0,
            peak_medium_db: -3.0,
        }
    }
}

/// 分析进度信息
#[derive(Debug, Clone)]
pub struct AnalysisProgress {
    /// 当前处理的文件索引
    pub current_file: usize,
    /// 总文件数
    pub total_files: usize,
    /// 当前处理的文件路径
    pub current_path: String,
    /// 已完成的文件数
    pub completed_files: usize,
}

impl AnalysisProgress {
    /// 计算完成百分比
    pub fn percentage(&self) -> f64 {
        if self.total_files == 0 {
            0.0
        } else {
            (self.completed_files as f64 / self.total_files as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_metrics_creation() {
        let metrics = AudioMetrics::new("test.wav".to_string(), 1024);
        assert_eq!(metrics.file_path, "test.wav");
        assert_eq!(metrics.file_size_bytes, 1024);
        assert!(!metrics.is_complete());
    }

    #[test]
    fn test_audio_metrics_filename() {
        let metrics = AudioMetrics::new("/path/to/test.wav".to_string(), 1024);
        assert_eq!(metrics.filename(), "test.wav");
    }

    #[test]
    fn test_quality_thresholds_default() {
        let thresholds = QualityThresholds::default();
        assert_eq!(thresholds.spectrum_fake_threshold, -85.0);
        assert_eq!(thresholds.lra_excellent_min, 8.0);
    }

    #[test]
    fn test_analysis_progress_percentage() {
        let progress = AnalysisProgress {
            current_file: 5,
            total_files: 10,
            current_path: "test.wav".to_string(),
            completed_files: 5,
        };
        assert_eq!(progress.percentage(), 50.0);
    }
}
