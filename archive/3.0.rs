use anyhow::{anyhow, Result};
use chrono::Local;
use lazy_static::lazy_static;
use rayon::prelude::*;
use regex::Regex;
use serde::Serialize;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
// 重要的补充：引入 Stdio 用于控制子进程的输入输出
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use walkdir::WalkDir;

// --- 预编译正则表达式（无变化）---
lazy_static! {
    static ref EBUR128_LRA_REGEX: Regex = Regex::new(r"LRA:\s*([0-9.-]+)\s*LU").unwrap();
    static ref EBUR128_SUMMARY_LRA_REGEX: Regex = Regex::new(r"(?m)^LRA:\s*([0-9.-]+)\s*LU\s*$").unwrap();
    static ref ASTATS_OVERALL_REGEX: Regex = Regex::new(
        r"(?m)^\[Parsed_astats_0 @ [^\]]+\] Overall\s*\n(?:[^\n]*\n)*?[^\n]*Peak level dB:\s*([-\d.]+)\s*\n(?:[^\n]*\n)*?[^\n]*RMS level dB:\s*([-\d.]+)"
    ).unwrap();
    static ref SIMPLE_PEAK_REGEX: Regex = Regex::new(r"Peak level dB:\s*([-\d.]+)").unwrap();
    static ref SIMPLE_RMS_REGEX: Regex = Regex::new(r"RMS level dB:\s*([-\d.]+)").unwrap();
    static ref HIGHPASS_ASTATS_REGEX: Regex = Regex::new(
        r"(?m)^\[Parsed_astats_1 @ [^\]]+\] Overall\s*\n(?:[^\n]*\n)*?[^\n]*RMS level dB:\s*([-\d.]+)"
    ).unwrap();
}

// --- 常量定义（无变化）---
const SUPPORTED_EXTENSIONS: [&str; 10] = [
    "wav", "mp3", "m4a", "flac", "aac", "ogg", "opus", "wma", "aiff", "alac",
];

// --- 数据结构定义（无变化）---
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

#[derive(Debug)]
struct AudioStats {
    peak_db: Option<f64>,
    rms_db: Option<f64>,
}

// --- 主程序逻辑 ---
fn main() -> Result<()> {
    println!("欢迎使用音频质量分析器 v3.1 (混合部署版)");
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
            // ... (内部逻辑无变化)
            let count = processed_count.fetch_add(1, Ordering::SeqCst) + 1;
            println!(
                "[线程 {:?}] ({}/{}) 正在处理: {}",
                std::thread::current().id(),
                count,
                total_files,
                path.display()
            );
            match process_file(&path) {
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
    println!("成功处理: {}/{} 个文件", results.len(), total_files);

    // 定义中间JSON文件的路径，它位于用户指定的文件夹内
    let json_output_path = base_folder_path.join("analysis_data.json");
    println!("正在将中间数据写入到: {}", json_output_path.display());
    let json_string = serde_json::to_string_pretty(&results)?;
    fs::write(&json_output_path, json_string)?;
    println!("中间数据写入成功！");

    // *** 核心修改部分：调用Python分析器并处理路径 ***
    println!("\n--- 即将启动 Python 分析模块 ---");

    let exe_path = std::env::current_exe()?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| anyhow!("无法找到可执行文件的父目录"))?;
    let analyzer_path = exe_dir.join("../Resources/ana_aud_analyzer");

    if !analyzer_path.exists() {
        return Err(anyhow!(
            "关键错误: 未能在预期位置找到分析器模块: {}. 请检查应用打包是否完整。",
            analyzer_path.display()
        ));
    }

    // 定义最终CSV报告的输出路径，同样位于用户指定的文件夹内
    let csv_output_path = base_folder_path.join("audio_quality_report.csv");

    println!("分析模块路径: {}", analyzer_path.display());
    println!("输入数据路径: {}", json_output_path.display());
    println!("输出报告路径: {}", csv_output_path.display());

    // 创建并配置Command
    let mut command = Command::new(&analyzer_path);
    command.arg(json_output_path.to_str().ok_or(anyhow!("JSON路径无效"))?); // 参数1: 输入的JSON文件
    command.arg("-o"); // 参数2: -o 标志
    command.arg(csv_output_path.to_str().ok_or(anyhow!("CSV路径无效"))?); // 参数3: 输出的CSV文件

    // **重要改进**：将子进程的输出和错误流直接连接到当前终端
    // 这样用户就能实时看到Python脚本的所有print()内容
    command.stdout(Stdio::inherit());
    command.stderr(Stdio::inherit());

    // 执行命令并等待其完成
    let status = command.status()?;

    if status.success() {
        println!("\n✅ Python 分析模块执行成功。");
    } else {
        // 使用 anyhow! 宏来创建一个包含上下文的错误
        return Err(anyhow!(
            "❌ Python 分析模块执行失败，退出代码: {:?}. 请检查上面的日志输出获取详细信息。",
            status.code()
        ));
    }

    println!("\n--- ✨ 全部分析流程完成 ---");
    println!(
        "最终报告已生成: {}",
        base_folder_path.join("audio_quality_report.csv").display()
    );
    println!("结束时间: {}", Local::now().format("%Y-%m-%d %H:%M:%S"));

    Ok(())
}

// --- 文件处理函数 (无变化) ---
fn process_file(path: &Path) -> Result<FileMetrics, String> {
    // ... 此函数内部逻辑保持不变 ...
    let start_time = std::time::Instant::now();
    let file_size_bytes = match fs::metadata(path) {
        Ok(meta) => meta.len(),
        Err(e) => return Err(format!("无法读取文件元数据: {}", e)),
    };
    let (lra_res, (stats_res, (rms_16k_res, (rms_18k_res, rms_20k_res)))) = rayon::join(
        || get_lra_ebur128_ffmpeg_fixed(path),
        || {
            rayon::join(
                || get_stats_ffmpeg_optimized(path),
                || {
                    rayon::join(
                        || get_highpass_rms_ffmpeg_optimized(path, 16000),
                        || {
                            rayon::join(
                                || get_highpass_rms_ffmpeg_optimized(path, 18000),
                                || get_highpass_rms_ffmpeg_optimized(path, 20000),
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

// --- FFmpeg相关函数 (无变化) ---
fn get_lra_ebur128_ffmpeg_fixed(path: &Path) -> Result<f64, String> {
    // ... 此函数内部逻辑保持不变 ...
    let mut command = Command::new("ffmpeg");
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

fn get_stats_ffmpeg_optimized(path: &Path) -> Result<AudioStats, String> {
    // ... 此函数内部逻辑保持不变 ...
    let mut command = Command::new("ffmpeg");
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

fn get_highpass_rms_ffmpeg_optimized(path: &Path, freq: u32) -> Result<f64, String> {
    // ... 此函数内部逻辑保持不变 ...
    let filter_str = format!("highpass=f={},astats=metadata=1", freq);
    let mut command = Command::new("ffmpeg");
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

// --- 辅助函数 (无变化) ---
fn run_command_and_get_stderr(mut command: Command) -> Result<String, String> {
    let output = command.stdin(Stdio::null()).stdout(Stdio::null()).output();
    match output {
        Ok(out) => Ok(String::from_utf8_lossy(&out.stderr).to_string()),
        Err(e) => Err(format!("无法执行命令: {}", e)),
    }
}

fn get_folder_path_from_user() -> Result<PathBuf> {
    // ... 此函数内部逻辑保持不变 ...
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
