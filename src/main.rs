use anyhow::{anyhow, Context, Result};
use chrono::Local;
use lazy_static::lazy_static;
use rayon::prelude::*;
use regex::Regex;
use serde::Serialize;
use std::fs::{self, File};
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt; // 在 macOS/Linux 上设置执行权限所需
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::TempDir; // 用于创建临时目录
use walkdir::WalkDir;

// --- 编译时嵌入依赖文件 ---
// Rust会读取这两个文件的二进制内容，并将其作为常量嵌入到最终的可执行文件中。
// 路径是相对于 Cargo.toml 所在的根目录。
const FFMPEG_BYTES: &[u8] = include_bytes!("../resources/ffmpeg");
const ANALYZER_BYTES: &[u8] = include_bytes!("../resources/ana_aud_analyzer");

// --- 预编译正则表达式 ---
lazy_static! {
    // EBU R128 LRA 提取正则（修复关键）
    static ref EBUR128_LRA_REGEX: Regex = Regex::new(r"LRA:\s*([0-9.-]+)\s*LU").unwrap();

    // EBU R128 汇总 LRA 提取正则（备用）
    static ref EBUR128_SUMMARY_LRA_REGEX: Regex = Regex::new(r"(?m)^LRA:\s*([0-9.-]+)\s*LU\s*$").unwrap();

    // 基础统计信息提取正则（兼容性优化）
    static ref ASTATS_OVERALL_REGEX: Regex = Regex::new(
        r"(?m)^\[Parsed_astats_0 @ [^\]]+\] Overall\s*\n(?:[^\n]*\n)*?[^\n]*Peak level dB:\s*([-\d.]+)\s*\n(?:[^\n]*\n)*?[^\n]*RMS level dB:\s*([-\d.]+)"
    ).unwrap();

    // 备用简单正则表达式
    static ref SIMPLE_PEAK_REGEX: Regex = Regex::new(r"Peak level dB:\s*([-\d.]+)").unwrap();
    static ref SIMPLE_RMS_REGEX: Regex = Regex::new(r"RMS level dB:\s*([-\d.]+)").unwrap();

    // 高通滤波后的RMS提取正则（基于原版逻辑）
    static ref HIGHPASS_ASTATS_REGEX: Regex = Regex::new(
        r"(?m)^\[Parsed_astats_1 @ [^\]]+\] Overall\s*\n(?:[^\n]*\n)*?[^\n]*RMS level dB:\s*([-\d.]+)"
    ).unwrap();
}

// --- 数据结构定义 ---
#[derive(Debug, Serialize)]
struct FileMetrics {
    #[serde(rename = "filePath")]
    file_path: String,
    #[serde(rename = "fileSizeBytes")]
    file_size_bytes: u64,
    #[serde(rename = "lra")]
    lra: Option<f64>,
    #[serde(rename = "peakAmplitudeDb")]
    peak_amplitude_db: Option<f64>,
    #[serde(rename = "overallRmsDb")]
    overall_rms_db: Option<f64>,
    #[serde(rename = "rmsDbAbove16k")]
    rms_db_above_16k: Option<f64>,
    #[serde(rename = "rmsDbAbove18k")]
    rms_db_above_18k: Option<f64>,
    #[serde(rename = "rmsDbAbove20k")]
    rms_db_above_20k: Option<f64>,
    #[serde(rename = "processingTimeMs")]
    processing_time_ms: u64,
}

// 用于从 astats 中获取峰值和RMS（基于原版）
#[derive(Debug)]
struct AudioStats {
    peak_db: Option<f64>,
    rms_db: Option<f64>,
}

// --- 新增：用于管理解压后的可执行文件路径的结构体 ---
struct AppHandle {
    ffmpeg_path: PathBuf,
    analyzer_path: PathBuf,
    // _temp_dir 必须被持有，因为当它被丢弃（drop）时，临时目录会自动被删除
    _temp_dir: TempDir,
}

// --- 常量定义 ---
const SUPPORTED_EXTENSIONS: [&str; 10] = [
    "wav", "mp3", "m4a", "flac", "aac", "ogg", "opus", "wma", "aiff", "alac",
];

/// 在临时目录中设置并准备可执行依赖项。
fn setup_dependencies() -> Result<AppHandle> {
    // 创建一个唯一的临时目录，程序结束时会自动清理
    let temp_dir = tempfile::Builder::new()
        .prefix("audio_analyzer_")
        .tempdir()?;

    println!("正在后台准备依赖项...");

    // 1. 准备 FFmpeg
    let ffmpeg_path = temp_dir.path().join("ffmpeg");
    let mut ffmpeg_file = File::create(&ffmpeg_path)?;
    ffmpeg_file.write_all(FFMPEG_BYTES)?;
    // 在macOS/Linux上，必须使其可执行
    let mut perms = ffmpeg_file.metadata()?.permissions();
    perms.set_mode(0o755); // rwxr-xr-x
    fs::set_permissions(&ffmpeg_path, perms)?;

    // 2. 准备 Python 分析器
    let analyzer_path = temp_dir.path().join("ana_aud_analyzer");
    let mut analyzer_file = File::create(&analyzer_path)?;
    analyzer_file.write_all(ANALYZER_BYTES)?;
    let mut perms = analyzer_file.metadata()?.permissions();
    perms.set_mode(0o755); // rwxr-xr-x
    fs::set_permissions(&analyzer_path, perms)?;

    println!("依赖项准备就绪。");

    Ok(AppHandle {
        ffmpeg_path,
        analyzer_path,
        _temp_dir: temp_dir,
    })
}

// --- 主程序逻辑 ---
fn main() -> Result<()> {
    // 在程序开始时，首先设置好我们的依赖环境
    let app_handle = setup_dependencies().context("初始化依赖环境失败")?;

    println!("欢迎使用音频质量分析器 v4.2 (单文件最终版)");
    println!("开始时间: {}", Local::now().format("%Y-%m-%d %H:%M:%S"));

    let base_folder_path = get_folder_path_from_user()?;
    println!("正在扫描文件夹: {}", base_folder_path.display());

    let files_to_process: Vec<PathBuf> = WalkDir::new(&base_folder_path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|path| {
            path.extension()
                .and_then(|s| s.to_str())
                .map(|ext| SUPPORTED_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .collect();

    if files_to_process.is_empty() {
        println!("在指定路径下没有找到支持的音频文件。");
        return Ok(());
    }

    let total_files = files_to_process.len();
    println!(
        "扫描完成，找到 {} 个音频文件待处理。开始并行分析...",
        total_files
    );

    let processed_count = AtomicUsize::new(0);
    let start_time = std::time::Instant::now();

    let results: Vec<FileMetrics> = files_to_process
        .into_par_iter()
        .filter_map(|path| {
            let count = processed_count.fetch_add(1, Ordering::SeqCst) + 1;
            println!(
                "[Rust 端数据提取] ({}/{}) 正在处理: {}",
                count,
                total_files,
                path.display()
            );
            // 将ffmpeg的路径传递给处理函数
            match process_file(&path, &app_handle.ffmpeg_path) {
                Ok(metrics) => Some(metrics),
                Err(e) => {
                    eprintln!(
                        "处理失败: {}\n └─> 错误详情: {}",
                        path.display(),
                        e.replace("\n", "\n ")
                    );
                    None
                }
            }
        })
        .collect();

    let processing_time = start_time.elapsed();
    println!("\n=== Rust 数据提取完成 ===");
    println!("总处理时间: {:.2}秒", processing_time.as_secs_f64());

    let json_output_path = base_folder_path.join("analysis_data.json");
    println!("正在将中间数据写入到: {}", json_output_path.display());
    fs::write(&json_output_path, serde_json::to_string_pretty(&results)?)?;
    println!("中间数据写入成功！");

    // 调用Python分析模块
    let csv_output_path = base_folder_path.join("audio_quality_report.csv");

    let mut command = Command::new(&app_handle.analyzer_path);
    command.arg(&json_output_path);
    command.arg("-o");
    command.arg(&csv_output_path);
    command.stdout(Stdio::inherit()).stderr(Stdio::inherit());

    let status = command.status().context("执行Python分析模块失败")?;

    if !status.success() {
        return Err(anyhow!(
            "Python分析模块执行异常，退出代码: {:?}",
            status.code()
        ));
    }

    println!("\n--- ✨ 全部分析流程完成 ---");
    println!("最终报告已生成: {}", csv_output_path.display());
    println!("结束时间: {}", Local::now().format("%Y-%m-%d %H:%M:%S"));

    Ok(())
}

// --- 文件处理函数 ---
fn process_file(path: &Path, ffmpeg_path: &Path) -> Result<FileMetrics, String> {
    let start_time = std::time::Instant::now();
    let file_size_bytes = fs::metadata(path)
        .map(|m| m.len())
        .map_err(|e| e.to_string())?;

    let (lra_res, (stats_res, (rms_16k_res, (rms_18k_res, rms_20k_res)))) = rayon::join(
        || get_lra_ebur128_ffmpeg_fixed(path, ffmpeg_path),
        || {
            rayon::join(
                || get_stats_ffmpeg_optimized(path, ffmpeg_path),
                || {
                    rayon::join(
                        || get_highpass_rms_ffmpeg_optimized(path, 16000, ffmpeg_path),
                        || {
                            rayon::join(
                                || get_highpass_rms_ffmpeg_optimized(path, 18000, ffmpeg_path),
                                || get_highpass_rms_ffmpeg_optimized(path, 20000, ffmpeg_path),
                            )
                        },
                    )
                },
            )
        },
    );

    let processing_time_ms = start_time.elapsed().as_millis() as u64;

    let metrics = FileMetrics {
        file_path: path.to_string_lossy().into_owned(),
        file_size_bytes,
        lra: lra_res.ok(),
        peak_amplitude_db: stats_res.as_ref().ok().and_then(|s| s.peak_db),
        overall_rms_db: stats_res.as_ref().ok().and_then(|s| s.rms_db),
        rms_db_above_16k: rms_16k_res.ok(),
        rms_db_above_18k: rms_18k_res.ok(),
        rms_db_above_20k: rms_20k_res.ok(),
        processing_time_ms,
    };

    Ok(metrics)
}

// --- FFmpeg相关函数 ---
fn run_command_and_get_stderr(mut command: Command) -> Result<String, String> {
    let output = command.stdin(Stdio::null()).stdout(Stdio::null()).output();
    match output {
        Ok(out) => Ok(String::from_utf8_lossy(&out.stderr).to_string()),
        Err(e) => Err(format!("无法执行命令: {}", e)),
    }
}

fn get_lra_ebur128_ffmpeg_fixed(path: &Path, ffmpeg_path: &Path) -> Result<f64, String> {
    let mut command = Command::new(ffmpeg_path);
    command
        .arg("-i")
        .arg(path)
        .arg("-filter_complex")
        .arg("ebur128")
        .arg("-f")
        .arg("null")
        .arg("-")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("info");

    let stderr = run_command_and_get_stderr(command)?;

    if let Some(caps) = EBUR128_SUMMARY_LRA_REGEX.captures(&stderr) {
        if let Some(lra_str) = caps.get(1) {
            if let Ok(lra_value) = lra_str.as_str().parse::<f64>() {
                return Ok(lra_value);
            }
        }
    }

    let lra_values: Vec<f64> = EBUR128_LRA_REGEX
        .captures_iter(&stderr)
        .filter_map(|caps| caps.get(1))
        .filter_map(|m| m.as_str().parse::<f64>().ok())
        .collect();

    if let Some(&last_lra) = lra_values.last() {
        Ok(last_lra)
    } else {
        Err(format!(
            "无法从ebur128输出中解析LRA值. Stderr preview: {}",
            stderr.chars().take(500).collect::<String>()
        ))
    }
}

fn get_stats_ffmpeg_optimized(path: &Path, ffmpeg_path: &Path) -> Result<AudioStats, String> {
    let mut command = Command::new(ffmpeg_path);
    command
        .arg("-i")
        .arg(path)
        .arg("-filter:a")
        .arg("astats=metadata=1")
        .arg("-map")
        .arg("0:a")
        .arg("-f")
        .arg("null")
        .arg("-")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("info");

    let stderr = run_command_and_get_stderr(command)?;

    if let Some(caps) = ASTATS_OVERALL_REGEX.captures(&stderr) {
        let peak_db = caps.get(1).and_then(|m| m.as_str().parse::<f64>().ok());
        let rms_db = caps.get(2).and_then(|m| m.as_str().parse::<f64>().ok());
        return Ok(AudioStats { peak_db, rms_db });
    }

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
        Err(format!(
            "无法从astats输出中解析峰值/RMS. Stderr: {}",
            stderr.trim()
        ))
    }
}

fn get_highpass_rms_ffmpeg_optimized(
    path: &Path,
    freq: u32,
    ffmpeg_path: &Path,
) -> Result<f64, String> {
    let mut command = Command::new(ffmpeg_path);
    let filter_str = format!("highpass=f={},astats=metadata=1", freq);
    command
        .arg("-i")
        .arg(path)
        .arg("-filter:a")
        .arg(&filter_str)
        .arg("-map")
        .arg("0:a")
        .arg("-f")
        .arg("null")
        .arg("-")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("info");

    let stderr = run_command_and_get_stderr(command)?;

    if let Some(caps) = HIGHPASS_ASTATS_REGEX.captures(&stderr) {
        if let Some(rms_str) = caps.get(1) {
            if let Ok(rms_value) = rms_str.as_str().parse::<f64>() {
                return Ok(rms_value);
            }
        }
    }

    let rms_values: Vec<f64> = SIMPLE_RMS_REGEX
        .captures_iter(&stderr)
        .filter_map(|caps| caps.get(1))
        .filter_map(|m| m.as_str().parse::<f64>().ok())
        .collect();

    if let Some(&last_rms) = rms_values.last() {
        Ok(last_rms)
    } else {
        Ok(-144.0)
    }
}

// --- 辅助函数 ---
fn get_folder_path_from_user() -> Result<PathBuf> {
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
