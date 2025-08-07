# API 文档

音频质量分析器提供了完整的 Rust API，可以作为库集成到其他项目中。

## 核心模块

### AudioAnalyzer

主要的音频分析器类，提供完整的音频质量分析功能。

```rust
use audio_analyzer_ultimate::{AudioAnalyzer, AnalyzerConfig};

// 创建分析器实例
let config = AnalyzerConfig::default();
let mut analyzer = AudioAnalyzer::new(config)?;

// 初始化依赖项
analyzer.initialize_dependencies()?;

// 分析单个文件
let metrics = analyzer.analyze_file(&path)?;

// 分析整个目录
let results = analyzer.analyze_directory(&dir_path)?;
```

#### 方法

##### `new(config: AnalyzerConfig) -> Result<Self>`

创建新的分析器实例。

**参数:**
- `config`: 分析器配置

**返回值:**
- `Ok(AudioAnalyzer)`: 成功创建的分析器实例
- `Err(AnalyzerError)`: 配置验证失败

##### `initialize_dependencies(&mut self) -> Result<()>`

初始化嵌入的依赖项（FFmpeg 和 Python 分析器）。

**注意:** 必须在进行任何分析操作之前调用此方法。

##### `analyze_file(&self, file_path: &Path) -> Result<AudioMetrics>`

分析单个音频文件。

**参数:**
- `file_path`: 音频文件路径

**返回值:**
- `Ok(AudioMetrics)`: 分析结果
- `Err(AnalyzerError)`: 分析失败

##### `analyze_directory<P: AsRef<Path>>(&self, dir_path: P) -> Result<Vec<AudioMetrics>>`

分析目录中的所有音频文件。

**参数:**
- `dir_path`: 目录路径

**返回值:**
- `Ok(Vec<AudioMetrics>)`: 所有文件的分析结果
- `Err(AnalyzerError)`: 分析失败

### AnalyzerConfig

分析器配置结构，用于自定义分析行为。

```rust
use audio_analyzer_ultimate::AnalyzerConfig;

let mut config = AnalyzerConfig::default();
config.verbose = true;
config.num_threads = Some(4);
config.show_progress = true;
```

#### 字段

- `supported_extensions: Vec<String>` - 支持的音频文件扩展名
- `quality_thresholds: QualityThresholds` - 质量评估阈值
- `num_threads: Option<usize>` - 并行线程数（None 表示使用系统默认）
- `verbose: bool` - 是否启用详细日志
- `show_progress: bool` - 是否显示进度信息
- `output: OutputConfig` - 输出配置
- `ffmpeg: FfmpegConfig` - FFmpeg 配置

#### 方法

##### `default() -> Self`

创建默认配置。

##### `from_file<P: AsRef<Path>>(path: P) -> Result<Self>`

从 TOML 文件加载配置。

##### `save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()>`

保存配置到 TOML 文件。

##### `validate(&self) -> Result<()>`

验证配置的有效性。

### AudioMetrics

音频分析结果结构。

```rust
use audio_analyzer_ultimate::AudioMetrics;

// 创建新的指标实例
let mut metrics = AudioMetrics::new("test.wav".to_string(), 1024);

// 检查数据完整性
if metrics.is_complete() {
    println!("分析数据完整");
}

// 获取文件名
let filename = metrics.filename();
```

#### 字段

- `file_path: String` - 文件路径
- `file_size_bytes: u64` - 文件大小（字节）
- `lra: Option<f64>` - 响度范围 (LU)
- `peak_amplitude_db: Option<f64>` - 峰值振幅 (dB)
- `overall_rms_db: Option<f64>` - 整体RMS电平 (dB)
- `rms_db_above_16k: Option<f64>` - 16kHz以上RMS (dB)
- `rms_db_above_18k: Option<f64>` - 18kHz以上RMS (dB)
- `rms_db_above_20k: Option<f64>` - 20kHz以上RMS (dB)
- `processing_time_ms: u64` - 处理时间（毫秒）

#### 方法

##### `new(file_path: String, file_size_bytes: u64) -> Self`

创建新的音频指标实例。

##### `is_complete(&self) -> bool`

检查是否包含所有必需的分析数据。

##### `filename(&self) -> String`

获取不含路径的文件名。

## 工具函数

### fs_utils

文件系统相关工具函数。

```rust
use audio_analyzer_ultimate::utils::fs_utils;

// 扫描音频文件
let extensions = vec!["wav".to_string(), "mp3".to_string()];
let files = fs_utils::scan_audio_files("/path/to/music", &extensions)?;

// 检查文件格式
let is_audio = fs_utils::is_supported_audio_file(&path, &extensions);

// 获取文件大小
let size = fs_utils::get_file_size(&path)?;
```

### string_utils

字符串处理工具函数。

```rust
use audio_analyzer_ultimate::utils::string_utils;

// 格式化文件大小
let size_str = string_utils::format_file_size(1048576); // "1.0 MB"

// 格式化持续时间
let duration_str = string_utils::format_duration(std::time::Duration::from_secs(90)); // "1m 30s"

// 截断字符串
let truncated = string_utils::truncate_string("very long string", 10); // "very lo..."
```

### Timer

性能测量工具。

```rust
use audio_analyzer_ultimate::utils::Timer;

let timer = Timer::new("操作名称");
// 执行一些操作...
timer.print_elapsed(); // 打印经过的时间
let duration = timer.stop(); // 停止并返回持续时间
```

## 错误处理

所有可能失败的操作都返回 `Result<T, AnalyzerError>`。

```rust
use audio_analyzer_ultimate::{AnalyzerError, Result};

match analyzer.analyze_file(&path) {
    Ok(metrics) => println!("分析成功: {:?}", metrics),
    Err(AnalyzerError::UnsupportedFormat { path, extension }) => {
        println!("不支持的格式: {} ({})", path, extension.unwrap_or_default());
    }
    Err(AnalyzerError::FfmpegError { message, stderr }) => {
        println!("FFmpeg 错误: {}", message);
        if let Some(stderr) = stderr {
            println!("详细信息: {}", stderr);
        }
    }
    Err(e) => println!("其他错误: {}", e),
}
```

## 示例

完整的使用示例请参考 [examples](../examples/) 目录。
