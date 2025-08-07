# 音频质量分析器 (Audio Quality Analyzer) v4.0.0

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![Python](https://img.shields.io/badge/python-3.8+-blue.svg)](https://www.python.org)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Version](https://img.shields.io/badge/version-4.0.0-brightgreen.svg)](CHANGELOG.md)

一个高性能的音频质量分析工具，采用 Rust + Python 混合架构，专为音频工程师和音乐制作人设计。

## 🎯 项目概述

本项目提供全面的音频质量分析功能，能够检测音频文件中的各种质量问题，包括：

- **动态范围分析** - 基于 EBU R128 标准的 LRA (Loudness Range) 测量
- **频谱完整性检测** - 识别伪造、升频和过度处理的音频
- **削波检测** - 检测数字削波和过载问题
- **批量处理** - 高效的并行处理大量音频文件
- **详细报告** - 生成包含质量评分和建议的 CSV 报告

## 🏗️ 架构设计

项目采用现代化的混合架构：

1. **Rust 核心引擎** - 负责音频数据提取和并行处理
   - 高性能的 FFmpeg 集成
   - 内存安全的并发处理
   - 嵌入式依赖管理

2. **Python 分析模块** - 负责数据分析和报告生成
   - 灵活的质量评估算法
   - 丰富的数据处理功能
   - 可扩展的报告格式

这种架构充分利用了 Rust 的性能优势和 Python 的生态系统。

## 📁 项目结构

```
audio-analyzer/
├── 📄 Cargo.toml              # Rust 项目配置
├── 📄 README.md               # 项目说明文档
├── 📄 requirements.txt        # Python 依赖列表
├── 📄 audio-analyzer.spec     # PyInstaller 配置文件
│
├── 📂 src/                    # 源代码目录
│   ├── 📄 lib.rs             # Rust 库入口
│   ├── 📂 bin/               # 可执行文件源码
│   │   ├── 📄 main.rs        # Rust 主程序
│   │   └── 📄 audio_analyzer.py # Python 分析模块
│   ├── 📄 analyzer.rs        # 音频分析核心模块
│   ├── 📄 config.rs          # 配置管理模块
│   ├── 📄 error.rs           # 错误处理模块
│   ├── 📄 types.rs           # 数据类型定义
│   └── 📄 utils.rs           # 工具函数模块
│
├── 📂 tests/                  # 测试套件
│   ├── 📄 lib.rs             # 测试入口
│   ├── 📂 unit/              # 单元测试
│   ├── 📂 integration/       # 集成测试
│   └── 📂 fixtures/          # 测试数据
│
├── 📂 docs/                   # 文档目录
│   ├── 📂 api/               # API 文档
│   ├── 📂 guides/            # 使用指南
│   └── 📂 examples/          # 示例代码
│
├── 📂 assets/                 # 资源文件
│   ├── 📂 binaries/          # 嵌入的二进制文件
│   │   ├── 📄 ffmpeg         # FFmpeg 可执行文件
│   │   └── 📄 audio-analyzer # Python 分析器可执行文件
│   └── 📂 sample_data/       # 示例音频文件
│
├── 📂 examples/               # 使用示例
├── 📂 archive/                # 历史版本备份
└── 📂 target/                 # Rust 编译输出（自动生成）
```

## 🚀 快速开始

### 系统要求

- **操作系统**: macOS 10.15+, Linux, Windows 10+
- **Rust**: 1.70 或更高版本
- **Python**: 3.8 或更高版本（用于开发）
- **内存**: 建议 4GB 以上
- **存储**: 至少 100MB 可用空间

### 支持的音频格式

| 格式 | 扩展名 | 说明 |
|------|--------|------|
| WAV | `.wav` | 无损音频，推荐用于分析 |
| FLAC | `.flac` | 无损压缩，高质量 |
| MP3 | `.mp3` | 有损压缩，常见格式 |
| AAC | `.aac`, `.m4a` | 高效压缩格式 |
| OGG | `.ogg`, `.opus` | 开源压缩格式 |
| 其他 | `.wma`, `.aiff`, `.alac` | 其他常见格式 |

### 安装方法

#### 方法一：预编译版本（推荐）

1. 从 [Releases](https://github.com/your-repo/audio-analyzer/releases) 下载最新版本
2. 解压到任意目录
3. 运行 `audio-analyzer` 可执行文件

#### 方法二：UV 快捷部署（推荐）

使用 UV 工具进行极速部署，安装速度提升 10-100 倍：

```bash
# 1. 克隆仓库
git clone https://github.com/your-repo/audio-analyzer.git
cd audio-analyzer

# 2. 一键部署（自动安装 UV 并构建）
./scripts/deploy-uv.sh

# 3. 验证安装
./target/release/audio-analyzer --help
```

**性能对比**：

| 操作 | 传统方式 (pip) | UV 方式 | 提升倍数 |
|------|----------------|---------|----------|
| 依赖安装 | 45-60s | 3-5s | 10-15x |
| 虚拟环境创建 | 8-12s | 1-2s | 6-8x |
| 总构建时间 | 80-120s | 15-25s | 4-6x |

#### 方法三：传统方式编译

```bash
# 1. 克隆仓库
git clone https://github.com/your-repo/audio-analyzer.git
cd audio-analyzer

# 2. 使用增强的构建脚本
./scripts/build.sh --clean --package

# 或者手动编译
cargo build --release
pip install -r requirements.txt
pyinstaller audio-analyzer.spec

# 5. 移动生成的文件到正确位置
mv dist/audio-analyzer assets/binaries/

# 6. 运行程序
./target/release/audio-analyzer
```

## 📖 使用指南

### 基本使用

1. **启动程序**
   ```bash
   ./audio-analyzer
   ```

2. **输入音频文件夹路径**
   ```
   请输入要递归处理的音乐顶层文件夹路径: /path/to/your/music
   ```

3. **等待分析完成**
   程序会自动：
   - 扫描指定目录下的所有音频文件
   - 并行分析每个文件的质量指标
   - 生成详细的分析报告

4. **查看结果**
   分析完成后会生成两个文件：
   - `analysis_data.json` - 原始分析数据
   - `audio_quality_report.csv` - 格式化的质量报告

### 环境变量配置

可以通过环境变量自定义程序行为：

```bash
# 启用详细输出
export AUDIO_ANALYZER_VERBOSE=true

# 设置并行线程数（默认为CPU核心数）
export AUDIO_ANALYZER_THREADS=8

# 运行程序
./audio-analyzer
```

### 输出报告说明

生成的 CSV 报告包含以下列：

| 列名 | 说明 | 单位 |
|------|------|------|
| 质量分 | 综合质量评分 | 0-100 |
| 状态 | 质量状态描述 | 文本 |
| filePath | 文件路径 | 路径 |
| 备注 | 详细分析说明 | 文本 |
| lra | 响度范围 | LU |
| peakAmplitudeDb | 峰值电平 | dB |
| rmsDbAbove16k | 16kHz以上RMS | dB |
| rmsDbAbove18k | 18kHz以上RMS | dB |
| rmsDbAbove20k | 20kHz以上RMS | dB |

### 质量评估标准

#### 动态范围 (LRA)
- **0-3 LU**: 🔴 严重压缩 - 动态范围极低，严重过度压缩
- **3-6 LU**: 🟡 低动态 - 动态范围过低，可能过度压缩
- **8-12 LU**: 🟢 优秀 - 理想的动态范围
- **15+ LU**: 🟡 过高 - 动态范围过高，可能需要压缩处理

#### 频谱完整性
- **高于 -70dB**: 🟢 完整 - 频谱完整，未发现处理痕迹
- **-80dB 到 -70dB**: 🟡 疑似处理 - 可能存在软性截止
- **低于 -85dB**: 🔴 可疑伪造 - 高度疑似伪造或升频

#### 峰值电平
- **低于 -6dB**: 🟢 安全 - 峰值电平安全，无削波风险
- **-6dB 到 -3dB**: 🟡 注意 - 峰值较高，需要注意
- **高于 -0.1dB**: 🔴 削波 - 存在数字削波风险

## 🛠️ 开发指南

### 项目清理

在开发过程中或准备发布时，可以使用项目提供的清理脚本清理构建产物和临时文件：

```bash
# 预览将要清理的文件
./scripts/clean-for-release.sh --dry-run

# 交互式清理（推荐）
./scripts/clean-for-release.sh

# 自动清理（非交互模式）
./scripts/clean-for-release.sh -y

# 查看清理选项
./scripts/clean-for-release.sh --help
```

**清理内容：**
- 构建产物：`target/`, `build/`, `dist/`, `releases/`
- Python 缓存：`__pycache__/`, `*.pyc`, `.pytest_cache/`
- 系统临时文件：`.DS_Store`, `Thumbs.db` 等
- 环境文件：`.venv/`, `uv.lock`, `*.egg-info/`
- 缓存文件：`.cache/`, `.mypy_cache/` 等

**保留的重要文件：**
- 源代码和配置文件
- 文档和 README
- FFmpeg 二进制文件 (`assets/binaries/ffmpeg`)

### 测试和验证

清理后验证项目仍可正常构建：

```bash
# 使用 UV 快速部署验证
./scripts/deploy-uv.sh --skip-tests

# 或使用传统方式验证
./scripts/build.sh

# 运行测试套件
cargo test
```
