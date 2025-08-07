# 部署指南

本文档提供音频质量分析器的部署和发布最佳实践。

## 目录

1. [构建准备](#构建准备)
2. [UV 快捷部署](#uv-快捷部署)
3. [本地构建](#本地构建)
4. [CI/CD 集成](#cicd-集成)
5. [发布流程](#发布流程)
6. [部署环境](#部署环境)
7. [故障排除](#故障排除)

## 构建准备

### 系统要求

**开发环境：**
- Rust 1.70+
- Python 3.8+
- Git
- 至少 2GB 可用内存
- 至少 1GB 可用磁盘空间

**目标平台：**
- macOS 10.15+ (x86_64, ARM64)
- Linux (x86_64, ARM64)
- Windows 10+ (x86_64)

### 依赖项检查

```bash
# 检查 Rust 版本
rustc --version
cargo --version

# 检查 Python 版本
python3 --version
pip3 --version

# 安装 Python 构建依赖
pip3 install -r requirements.txt
pip3 install pyinstaller
```

## UV 快捷部署

### 什么是 UV 快捷部署

UV 是一个极快的 Python 包管理器，可以显著提升构建和部署速度。音频质量分析器 v4.0 集成了完整的 UV 支持。

### 一键部署

```bash
# 基本部署（自动检测和安装 UV）
./scripts/deploy-uv.sh

# 完整部署（包含所有检查）
./scripts/deploy-uv.sh --clean

# 快速部署（跳过质量检查和测试）
./scripts/deploy-uv.sh --skip-quality --skip-tests
```

### 部署流程

UV 快捷部署脚本会自动执行以下步骤：

1. **环境检测**: 自动检测操作系统和 UV 可用性
2. **UV 安装**: 如果未安装，自动下载并安装 UV
3. **虚拟环境**: 创建和配置 Python 虚拟环境
4. **依赖安装**: 使用 UV 快速安装所有依赖
5. **代码检查**: 运行格式化、类型检查等质量检查
6. **构建**: 编译 Rust 主程序和 Python 分析器
7. **测试**: 运行完整测试套件
8. **验证**: 执行健康检查确保构建正确

### 性能优势

| 操作 | 传统方式 | UV 方式 | 提升 |
|------|----------|---------|------|
| 依赖安装 | 45-60s | 3-5s | 10-15x |
| 环境创建 | 8-12s | 1-2s | 6-8x |
| 总部署时间 | 80-120s | 15-25s | 4-6x |

### 跨平台支持

#### macOS
```bash
# 自动安装 UV
./scripts/deploy-uv.sh --install-uv

# 验证部署
./target/release/audio-analyzer --version
```

#### Linux
```bash
# 确保系统依赖
sudo apt-get update && sudo apt-get install -y curl build-essential

# 运行部署
./scripts/deploy-uv.sh
```

#### Windows
```powershell
# 手动安装 UV（管理员模式）
powershell -c "irm https://astral.sh/uv/install.ps1 | iex"

# 运行部署（Git Bash 或 WSL）
./scripts/deploy-uv.sh
```

## 本地构建

### 快速构建

使用提供的构建脚本进行一键构建：

```bash
# 赋予执行权限
chmod +x scripts/build.sh

# 完整构建
./scripts/build.sh

# 清理后构建
./scripts/build.sh --clean

# 包含基准测试的构建
./scripts/build.sh --with-bench

# 创建发布包
./scripts/build.sh --package
```

### 手动构建步骤

如果需要手动控制构建过程：

#### 1. 构建 Python 分析器

```bash
# 安装依赖
pip3 install -r requirements.txt

# 构建可执行文件
pyinstaller --onefile \
            --name audio-analyzer \
            --clean \
            --distpath assets/binaries \
            src/bin/audio_analyzer.py

# 验证构建
ls -la assets/binaries/audio-analyzer
```

#### 2. 构建 Rust 主程序

```bash
# 运行测试
cargo test

# 构建发布版本
cargo build --release

# 验证构建
./target/release/audio-analyzer --help
```

#### 3. 验证完整功能

```bash
# 创建测试目录
mkdir -p test_audio

# 运行完整测试（需要音频文件）
./target/release/audio-analyzer test_audio
```

## CI/CD 集成

### GitHub Actions 配置

创建 `.github/workflows/build.yml`：

```yaml
name: Build and Test

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        rust: [stable]

    steps:
    - uses: actions/checkout@v3
    
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: ${{ matrix.rust }}
        override: true
        components: rustfmt, clippy

    - name: Setup Python
      uses: actions/setup-python@v4
      with:
        python-version: '3.9'

    - name: Install Python dependencies
      run: |
        python -m pip install --upgrade pip
        pip install -r requirements.txt
        pip install pyinstaller

    - name: Run tests
      run: cargo test --verbose

    - name: Run clippy
      run: cargo clippy -- -D warnings

    - name: Check formatting
      run: cargo fmt -- --check

    - name: Build release
      run: cargo build --release --verbose

    - name: Build Python analyzer
      run: |
        pyinstaller --onefile --name audio-analyzer src/bin/audio_analyzer.py
        mkdir -p assets/binaries
        cp dist/audio-analyzer assets/binaries/

    - name: Upload artifacts
      uses: actions/upload-artifact@v3
      with:
        name: audio-analyzer-${{ matrix.os }}
        path: |
          target/release/audio-analyzer*
          assets/binaries/audio-analyzer*
```

### 发布自动化

创建 `.github/workflows/release.yml`：

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  release:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: macos-latest
            target: x86_64-apple-darwin
          - os: windows-latest
            target: x86_64-pc-windows-msvc

    steps:
    - uses: actions/checkout@v3
    
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        target: ${{ matrix.target }}
        override: true

    - name: Build release
      run: |
        chmod +x scripts/build.sh
        ./scripts/build.sh --package

    - name: Create Release
      uses: softprops/action-gh-release@v1
      with:
        files: releases/*.tar.gz
      env:
        GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

## 发布流程

### 版本管理

1. **更新版本号**
   ```bash
   # 更新 Cargo.toml 中的版本
   vim Cargo.toml
   
   # 更新文档中的版本引用
   grep -r "v3.0" docs/ --include="*.md"
   ```

2. **创建发布标签**
   ```bash
   git tag -a v4.0.0 -m "Release version 4.0.0"
   git push origin v4.0.0
   ```

3. **生成变更日志**
   ```bash
   # 使用 git-cliff 或手动创建
   git log --oneline v3.0.0..HEAD > CHANGELOG.md
   ```

### 发布前清理

在创建 GitHub Release 之前，建议使用项目提供的清理脚本清理所有构建产物和临时文件：

```bash
# 预览将要清理的文件
./scripts/clean-for-release.sh --dry-run

# 交互式清理（推荐）
./scripts/clean-for-release.sh

# 自动清理（非交互模式）
./scripts/clean-for-release.sh -y
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

**清理后验证：**
```bash
# 验证项目仍可正常构建
./scripts/deploy-uv.sh --skip-tests
cargo test
```

### 发布检查清单

- [ ] 执行发布前清理
- [ ] 所有测试通过
- [ ] 基准测试性能符合预期
- [ ] 文档更新完整
- [ ] 版本号正确更新
- [ ] 构建脚本测试通过
- [ ] 多平台兼容性验证
- [ ] 安全扫描通过
- [ ] 发布说明准备完成

## 部署环境

### 生产环境推荐配置

**硬件要求：**
- CPU: 4核心以上
- 内存: 8GB 以上
- 存储: SSD，至少 100GB 可用空间
- 网络: 稳定的互联网连接

**软件环境：**
- 操作系统: Ubuntu 20.04 LTS 或更新版本
- 容器化: Docker (可选)
- 监控: 系统监控和日志收集

### Docker 部署

创建 `Dockerfile`：

```dockerfile
FROM ubuntu:22.04

# 安装系统依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# 创建应用用户
RUN useradd -m -s /bin/bash audioanalyzer

# 复制应用文件
COPY target/release/audio-analyzer /usr/local/bin/
COPY assets/ /opt/audio-analyzer/assets/
COPY docs/ /opt/audio-analyzer/docs/

# 设置权限
RUN chmod +x /usr/local/bin/audio-analyzer
RUN chown -R audioanalyzer:audioanalyzer /opt/audio-analyzer

# 切换到应用用户
USER audioanalyzer
WORKDIR /home/audioanalyzer

# 健康检查
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD audio-analyzer --help || exit 1

# 入口点
ENTRYPOINT ["audio-analyzer"]
```

构建和运行：

```bash
# 构建镜像
docker build -t audio-analyzer:v4.0.0 .

# 运行容器
docker run -v /path/to/audio:/data audio-analyzer:v4.0.0 /data
```

### 系统服务部署

创建 systemd 服务文件 `/etc/systemd/system/audio-analyzer.service`：

```ini
[Unit]
Description=Audio Quality Analyzer Service
After=network.target

[Service]
Type=oneshot
User=audioanalyzer
Group=audioanalyzer
ExecStart=/usr/local/bin/audio-analyzer /opt/audio-data
WorkingDirectory=/opt/audio-analyzer
Environment=AUDIO_ANALYZER_VERBOSE=true
Environment=AUDIO_ANALYZER_THREADS=4

[Install]
WantedBy=multi-user.target
```

启用和管理服务：

```bash
# 重新加载 systemd
sudo systemctl daemon-reload

# 启用服务
sudo systemctl enable audio-analyzer

# 启动服务
sudo systemctl start audio-analyzer

# 查看状态
sudo systemctl status audio-analyzer
```

## 故障排除

### 常见构建问题

**问题：Rust 编译失败**
```bash
# 解决方案：更新 Rust 工具链
rustup update stable
cargo clean
cargo build --release
```

**问题：Python 依赖安装失败**
```bash
# 解决方案：使用虚拟环境
python3 -m venv venv
source venv/bin/activate
pip install --upgrade pip
pip install -r requirements.txt
```

**问题：PyInstaller 构建失败**
```bash
# 解决方案：清理缓存并重试
rm -rf build/ dist/ __pycache__/
pyinstaller --clean --onefile src/bin/audio_analyzer.py
```

### 运行时问题

**问题：权限不足**
```bash
# 解决方案：检查文件权限
chmod +x target/release/audio-analyzer
chmod +x assets/binaries/audio-analyzer
```

**问题：依赖项未找到**
```bash
# 解决方案：检查嵌入的二进制文件
ls -la assets/binaries/
file assets/binaries/audio-analyzer
```

**问题：内存不足**
```bash
# 解决方案：调整线程数
export AUDIO_ANALYZER_THREADS=2
./target/release/audio-analyzer /path/to/audio
```

### 性能优化

**大文件处理优化：**
- 增加系统内存
- 使用 SSD 存储
- 调整并行线程数
- 分批处理文件

**网络部署优化：**
- 使用 CDN 分发
- 启用压缩传输
- 实施缓存策略
- 监控性能指标

## 安全考虑

### 构建安全

- 使用官方 Rust 和 Python 镜像
- 定期更新依赖项
- 扫描已知漏洞
- 验证构建产物完整性

### 部署安全

- 最小权限原则
- 网络访问控制
- 日志审计
- 定期安全更新

### 数据安全

- 输入验证
- 临时文件清理
- 敏感信息保护
- 访问日志记录

如需更多帮助，请参考项目文档或联系开发团队。
