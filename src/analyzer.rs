//! # 音频分析器核心模块
//!
//! 提供音频文件分析的核心功能，包括FFmpeg集成、并行处理和数据提取。

use crate::config::AnalyzerConfig;
use crate::error::{AnalyzerError, Result};
use crate::types::{AudioMetrics, AudioStats};
use crate::utils::{fs_utils, process_utils, Timer};

use lazy_static::lazy_static;
use rayon::prelude::*;
use regex::Regex;
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::TempDir;

// 预编译的正则表达式，用于解析FFmpeg输出
lazy_static! {
    /// EBU R128 LRA 提取正则表达式
    static ref EBUR128_LRA_REGEX: Regex =
        Regex::new(r"LRA:\s*([0-9.-]+)\s*LU").unwrap();

    /// EBU R128 汇总 LRA 提取正则表达式
    static ref EBUR128_SUMMARY_LRA_REGEX: Regex =
        Regex::new(r"(?m)^LRA:\s*([0-9.-]+)\s*LU\s*$").unwrap();

    /// 基础统计信息提取正则表达式
    static ref ASTATS_OVERALL_REGEX: Regex = Regex::new(
        r"(?m)^\[Parsed_astats_0 @ [^\]]+\] Overall\s*\n(?:[^\n]*\n)*?[^\n]*Peak level dB:\s*([-\d.]+)\s*\n(?:[^\n]*\n)*?[^\n]*RMS level dB:\s*([-\d.]+)"
    ).unwrap();

    /// 简单峰值提取正则表达式
    static ref SIMPLE_PEAK_REGEX: Regex =
        Regex::new(r"Peak level dB:\s*([-\d.]+)").unwrap();

    /// 简单RMS提取正则表达式
    static ref SIMPLE_RMS_REGEX: Regex =
        Regex::new(r"RMS level dB:\s*([-\d.]+)").unwrap();

    /// 高通滤波后的RMS提取正则表达式
    static ref HIGHPASS_ASTATS_REGEX: Regex = Regex::new(
        r"(?m)^\[Parsed_astats_1 @ [^\]]+\] Overall\s*\n(?:[^\n]*\n)*?[^\n]*RMS level dB:\s*([-\d.]+)"
    ).unwrap();
}

/// 嵌入的二进制依赖文件
const FFMPEG_BYTES: &[u8] = include_bytes!("../assets/binaries/ffmpeg");
const ANALYZER_BYTES: &[u8] = include_bytes!("../assets/binaries/audio-analyzer");

/// 音频分析器主结构
pub struct AudioAnalyzer {
    /// 配置信息
    config: AnalyzerConfig,
    /// 依赖项句柄
    dependencies: Option<DependencyHandle>,
}

/// 依赖项管理句柄
struct DependencyHandle {
    /// FFmpeg 可执行文件路径
    ffmpeg_path: PathBuf,
    /// Python 分析器可执行文件路径
    analyzer_path: PathBuf,
    /// 临时目录（保持引用以防止被删除）
    _temp_dir: TempDir,
}

impl AudioAnalyzer {
    /// 创建新的音频分析器实例
    pub fn new(config: AnalyzerConfig) -> Result<Self> {
        // 验证配置
        config.validate()?;

        Ok(Self {
            config,
            dependencies: None,
        })
    }

    /// 使用默认配置创建分析器
    pub fn with_default_config() -> Result<Self> {
        Self::new(AnalyzerConfig::default())
    }

    /// 初始化依赖项（解压嵌入的二进制文件）
    ///
    /// 性能优化：使用并行解压和优化的I/O操作
    pub fn initialize_dependencies(&mut self) -> Result<()> {
        if self.dependencies.is_some() {
            return Ok(()); // 已经初始化过了
        }

        let timer = Timer::new("依赖项初始化");

        // 创建临时目录
        let temp_dir = tempfile::Builder::new()
            .prefix("audio_analyzer_")
            .tempdir()
            .map_err(|e| AnalyzerError::DependencyError(format!("创建临时目录失败: {e}")))?;

        if self.config.verbose {
            println!("正在初始化依赖项...");
        }

        // 并行解压二进制文件以提高性能
        let ffmpeg_path = temp_dir.path().join("ffmpeg");
        let analyzer_path = temp_dir.path().join("audio_analyzer");

        let (ffmpeg_result, analyzer_result) = rayon::join(
            || self.extract_binary_optimized(FFMPEG_BYTES, &ffmpeg_path, "FFmpeg"),
            || self.extract_binary_optimized(ANALYZER_BYTES, &analyzer_path, "Python分析器"),
        );

        // 检查结果
        ffmpeg_result?;
        analyzer_result?;

        self.dependencies = Some(DependencyHandle {
            ffmpeg_path,
            analyzer_path,
            _temp_dir: temp_dir,
        });

        if self.config.verbose {
            timer.print_elapsed();
        }

        Ok(())
    }

    /// 解压二进制文件到指定路径（保留用于兼容性）
    #[allow(dead_code)]
    fn extract_binary(&self, bytes: &[u8], path: &Path, name: &str) -> Result<()> {
        let mut file = File::create(path)?;
        file.write_all(bytes)?;

        // 设置可执行权限 (Unix系统)
        #[cfg(unix)]
        {
            let mut perms = file.metadata()?.permissions();
            perms.set_mode(0o755); // rwxr-xr-x
            fs::set_permissions(path, perms)?;
        }

        if self.config.verbose {
            println!("已解压 {}: {}", name, path.display());
        }

        Ok(())
    }

    /// 优化的二进制文件解压方法
    ///
    /// 性能优化：
    /// - 使用缓冲写入减少系统调用
    /// - 预分配文件大小
    /// - 批量设置权限
    fn extract_binary_optimized(&self, bytes: &[u8], path: &Path, name: &str) -> Result<()> {
        use std::io::BufWriter;

        // 创建文件并预分配空间
        let file = File::create(path)?;
        file.set_len(bytes.len() as u64)?;

        // 使用缓冲写入提高性能
        let mut writer = BufWriter::with_capacity(64 * 1024, file); // 64KB 缓冲区
        std::io::Write::write_all(&mut writer, bytes)?;
        writer.flush()?;

        // 设置可执行权限 (Unix系统)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o755);
            fs::set_permissions(path, perms)?;
        }

        if self.config.verbose {
            println!("已解压 {}: {}", name, path.display());
        }

        Ok(())
    }

    /// 分析单个音频文件
    pub fn analyze_file(&self, file_path: &Path) -> Result<AudioMetrics> {
        let dependencies = self
            .dependencies
            .as_ref()
            .ok_or_else(|| AnalyzerError::DependencyError("依赖项未初始化".to_string()))?;

        let timer = Timer::new("文件分析");
        let file_size = fs_utils::get_file_size(file_path)?;

        // 并行执行多个分析任务
        let (lra_result, (stats_result, (rms_16k_result, (rms_18k_result, rms_20k_result)))) =
            rayon::join(
                || self.extract_lra_ebur128(file_path, &dependencies.ffmpeg_path),
                || {
                    rayon::join(
                        || self.extract_audio_stats(file_path, &dependencies.ffmpeg_path),
                        || {
                            rayon::join(
                                || {
                                    self.extract_highpass_rms(
                                        file_path,
                                        16000,
                                        &dependencies.ffmpeg_path,
                                    )
                                },
                                || {
                                    rayon::join(
                                        || {
                                            self.extract_highpass_rms(
                                                file_path,
                                                18000,
                                                &dependencies.ffmpeg_path,
                                            )
                                        },
                                        || {
                                            self.extract_highpass_rms(
                                                file_path,
                                                20000,
                                                &dependencies.ffmpeg_path,
                                            )
                                        },
                                    )
                                },
                            )
                        },
                    )
                },
            );

        let processing_time_ms = timer.elapsed().as_millis() as u64;

        let mut metrics = AudioMetrics::new(file_path.to_string_lossy().to_string(), file_size);

        // 设置分析结果
        metrics.lra = lra_result.ok();
        if let Ok(stats) = stats_result {
            metrics.peak_amplitude_db = stats.peak_db;
            metrics.overall_rms_db = stats.rms_db;
        }
        metrics.rms_db_above_16k = rms_16k_result.ok();
        metrics.rms_db_above_18k = rms_18k_result.ok();
        metrics.rms_db_above_20k = rms_20k_result.ok();
        metrics.processing_time_ms = processing_time_ms;

        Ok(metrics)
    }

    /// 批量分析音频文件
    pub fn analyze_files(&self, file_paths: &[PathBuf]) -> Result<Vec<AudioMetrics>> {
        if file_paths.is_empty() {
            return Ok(Vec::new());
        }

        let total_files = file_paths.len();
        let processed_count = Arc::new(AtomicUsize::new(0));

        if self.config.verbose {
            println!("开始并行分析 {total_files} 个文件...");
        }

        let timer = Timer::new("批量分析");

        let results: Vec<AudioMetrics> = file_paths
            .par_iter()
            .filter_map(|path| {
                let count = processed_count.fetch_add(1, Ordering::SeqCst) + 1;

                if self.config.show_progress {
                    println!(
                        "[{}/{}] 正在处理: {}",
                        count,
                        total_files,
                        fs_utils::get_display_name(path)
                    );
                }

                match self.analyze_file(path) {
                    Ok(metrics) => Some(metrics),
                    Err(e) => {
                        eprintln!("处理失败: {}\n └─> 错误详情: {}", path.display(), e);
                        None
                    }
                }
            })
            .collect();

        if self.config.verbose {
            timer.print_elapsed();
            println!("成功处理 {}/{} 个文件", results.len(), total_files);
        }

        Ok(results)
    }

    /// 分析目录中的所有音频文件
    pub fn analyze_directory<P: AsRef<Path>>(&self, dir_path: P) -> Result<Vec<AudioMetrics>> {
        let audio_files = fs_utils::scan_audio_files(dir_path, &self.config.supported_extensions)?;

        if audio_files.is_empty() {
            return Err(AnalyzerError::Other(
                "在指定目录中未找到支持的音频文件".to_string(),
            ));
        }

        if self.config.verbose {
            println!("找到 {} 个音频文件", audio_files.len());
        }

        self.analyze_files(&audio_files)
    }

    /// 使用 EBU R128 标准提取 LRA (Loudness Range)
    ///
    /// LRA (Loudness Range) 是衡量音频动态范围的重要指标，单位为LU (Loudness Units)。
    /// 该方法通过FFmpeg的ebur128滤镜来计算音频的响度范围。
    ///
    /// # 参数
    /// * `file_path` - 音频文件路径
    /// * `ffmpeg_path` - FFmpeg可执行文件路径
    ///
    /// # 返回值
    /// * `Ok(f64)` - 成功时返回LRA值（单位：LU）
    /// * `Err(AnalyzerError)` - 失败时返回错误信息
    ///
    /// # EBU R128标准说明
    /// EBU R128是欧洲广播联盟制定的音频响度标准，用于确保不同音频内容的响度一致性。
    /// LRA值通常在以下范围内：
    /// - 0-3 LU: 严重压缩，动态范围极低
    /// - 3-6 LU: 低动态范围，可能过度压缩
    /// - 8-12 LU: 理想的动态范围
    /// - >20 LU: 动态范围过高，可能需要压缩处理
    fn extract_lra_ebur128(&self, file_path: &Path, ffmpeg_path: &Path) -> Result<f64> {
        let mut command = Command::new(ffmpeg_path);
        command
            .arg("-i")
            .arg(file_path)
            .arg("-filter_complex")
            .arg("ebur128")
            .arg("-f")
            .arg("null")
            .arg("-");

        if self.config.ffmpeg.hide_banner {
            command.arg("-hide_banner");
        }
        command.arg("-loglevel").arg(&self.config.ffmpeg.log_level);

        let stderr = process_utils::run_command_capture_stderr(command)?;

        // 首先尝试匹配汇总的LRA值
        if let Some(caps) = EBUR128_SUMMARY_LRA_REGEX.captures(&stderr) {
            if let Some(lra_str) = caps.get(1) {
                if let Ok(lra_value) = lra_str.as_str().parse::<f64>() {
                    return Ok(lra_value);
                }
            }
        }

        // 如果没有找到汇总值，尝试提取所有LRA值并取最后一个
        let lra_values: Vec<f64> = EBUR128_LRA_REGEX
            .captures_iter(&stderr)
            .filter_map(|caps| caps.get(1))
            .filter_map(|m| m.as_str().parse::<f64>().ok())
            .collect();

        if let Some(&last_lra) = lra_values.last() {
            Ok(last_lra)
        } else {
            Err(AnalyzerError::ParseError {
                message: "无法从EBU R128输出中解析LRA值".to_string(),
                raw_data: Some(stderr.chars().take(500).collect()),
            })
        }
    }

    /// 提取音频统计信息（峰值和RMS）
    ///
    /// 使用FFmpeg的astats滤镜提取音频的基本统计信息，包括峰值电平和RMS电平。
    /// 这些指标用于评估音频的整体响度和是否存在削波等问题。
    ///
    /// # 参数
    /// * `file_path` - 音频文件路径
    /// * `ffmpeg_path` - FFmpeg可执行文件路径
    ///
    /// # 返回值
    /// * `Ok(AudioStats)` - 成功时返回音频统计信息
    /// * `Err(AnalyzerError)` - 失败时返回错误信息
    ///
    /// # 统计指标说明
    /// - **峰值电平 (Peak Level)**: 音频信号的最大振幅，单位为dB
    ///   - 接近0dB表示可能存在削波风险
    ///   - 低于-6dB通常被认为是安全的
    /// - **RMS电平 (RMS Level)**: 音频信号的有效值，反映平均响度
    ///   - 比峰值电平更能反映人耳感知的响度
    fn extract_audio_stats(&self, file_path: &Path, ffmpeg_path: &Path) -> Result<AudioStats> {
        let mut command = Command::new(ffmpeg_path);
        command
            .arg("-i")
            .arg(file_path)
            .arg("-filter:a")
            .arg("astats=metadata=1")
            .arg("-map")
            .arg("0:a")
            .arg("-f")
            .arg("null")
            .arg("-");

        if self.config.ffmpeg.hide_banner {
            command.arg("-hide_banner");
        }
        command.arg("-loglevel").arg(&self.config.ffmpeg.log_level);

        let stderr = process_utils::run_command_capture_stderr(command)?;

        // 尝试使用复杂正则表达式匹配
        if let Some(caps) = ASTATS_OVERALL_REGEX.captures(&stderr) {
            let peak_db = caps.get(1).and_then(|m| m.as_str().parse::<f64>().ok());
            let rms_db = caps.get(2).and_then(|m| m.as_str().parse::<f64>().ok());
            return Ok(AudioStats { peak_db, rms_db });
        }

        // 回退到简单正则表达式
        let peak_db = SIMPLE_PEAK_REGEX
            .captures(&stderr)
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse::<f64>().ok());

        let rms_db = SIMPLE_RMS_REGEX
            .captures(&stderr)
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse::<f64>().ok());

        if peak_db.is_some() || rms_db.is_some() {
            Ok(AudioStats { peak_db, rms_db })
        } else {
            Err(AnalyzerError::ParseError {
                message: "无法从astats输出中解析峰值/RMS".to_string(),
                raw_data: Some(stderr.trim().to_string()),
            })
        }
    }

    /// 提取高通滤波后的RMS值
    fn extract_highpass_rms(
        &self,
        file_path: &Path,
        frequency: u32,
        ffmpeg_path: &Path,
    ) -> Result<f64> {
        let mut command = Command::new(ffmpeg_path);
        let filter_str = format!("highpass=f={frequency},astats=metadata=1");

        command
            .arg("-i")
            .arg(file_path)
            .arg("-filter:a")
            .arg(&filter_str)
            .arg("-map")
            .arg("0:a")
            .arg("-f")
            .arg("null")
            .arg("-");

        if self.config.ffmpeg.hide_banner {
            command.arg("-hide_banner");
        }
        command.arg("-loglevel").arg(&self.config.ffmpeg.log_level);

        let stderr = process_utils::run_command_capture_stderr(command)?;

        // 尝试使用高通滤波专用正则表达式
        if let Some(caps) = HIGHPASS_ASTATS_REGEX.captures(&stderr) {
            if let Some(rms_str) = caps.get(1) {
                if let Ok(rms_value) = rms_str.as_str().parse::<f64>() {
                    return Ok(rms_value);
                }
            }
        }

        // 回退到简单RMS正则表达式
        let rms_values: Vec<f64> = SIMPLE_RMS_REGEX
            .captures_iter(&stderr)
            .filter_map(|caps| caps.get(1))
            .filter_map(|m| m.as_str().parse::<f64>().ok())
            .collect();

        if let Some(&last_rms) = rms_values.last() {
            Ok(last_rms)
        } else {
            // 如果没有找到任何RMS值，返回一个默认的低值
            Ok(-144.0)
        }
    }

    /// 获取配置的引用
    pub fn config(&self) -> &AnalyzerConfig {
        &self.config
    }

    /// 检查依赖项是否已初始化
    pub fn is_initialized(&self) -> bool {
        self.dependencies.is_some()
    }

    /// 获取Python分析器路径（如果已初始化）
    pub fn get_analyzer_path(&self) -> Option<&std::path::Path> {
        self.dependencies
            .as_ref()
            .map(|deps| deps.analyzer_path.as_path())
    }
}
