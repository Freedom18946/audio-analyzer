#!/bin/bash
# 音频质量分析器构建脚本
# 
# 此脚本用于构建完整的音频分析器项目，包括Rust主程序和Python分析模块

set -e  # 遇到错误时退出

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 日志函数
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 检查 UV 工具可用性
check_uv() {
    if command -v uv &> /dev/null; then
        log_info "检测到 UV 工具，将优先使用 UV 进行 Python 环境管理"
        USE_UV=true
        UV_VERSION=$(uv --version | cut -d' ' -f2)
        log_info "UV 版本: ${UV_VERSION}"
    else
        log_warning "UV 未安装，将使用传统的 pip 方式"
        USE_UV=false
    fi
}

# 检查依赖项
check_dependencies() {
    log_info "检查构建依赖项..."

    # 检查 Rust
    if ! command -v cargo &> /dev/null; then
        log_error "Cargo 未安装。请安装 Rust: https://rustup.rs/"
        exit 1
    fi

    # 检查 Python
    if ! command -v python3 &> /dev/null; then
        log_error "Python3 未安装。请安装 Python 3.8+"
        exit 1
    fi

    # 检查 UV 可用性
    check_uv

    # 根据 UV 可用性检查 PyInstaller
    if [ "$USE_UV" = true ]; then
        log_info "将使用 UV 管理 Python 依赖"
    else
        if ! command -v pyinstaller &> /dev/null; then
            log_warning "PyInstaller 未安装。正在安装..."
            pip3 install pyinstaller
        fi
    fi

    log_success "所有依赖项检查完成"
}

# 清理构建目录
clean_build() {
    log_info "清理构建目录..."
    
    # 清理 Rust 构建产物
    cargo clean
    
    # 清理 Python 构建产物
    rm -rf build/ dist/ *.spec
    
    # 清理临时文件
    find . -name "*.pyc" -delete
    find . -name "__pycache__" -type d -exec rm -rf {} + 2>/dev/null || true
    
    log_success "构建目录清理完成"
}

# 运行测试
run_tests() {
    log_info "运行测试套件..."
    
    # 运行 Rust 测试
    log_info "运行 Rust 测试..."
    cargo test --lib --tests --bins
    
    # 运行基准测试（可选）
    if [ "$1" = "--with-bench" ]; then
        log_info "运行基准测试..."
        cargo bench
    fi
    
    log_success "所有测试通过"
}

# 使用 UV 安装 Python 依赖
install_python_deps_uv() {
    log_info "使用 UV 安装 Python 依赖..."

    # 确保虚拟环境存在
    if [ ! -d ".venv" ]; then
        log_info "创建 UV 虚拟环境..."
        uv venv
    fi

    # 同步项目依赖
    log_info "同步项目依赖..."
    uv sync --all-extras

    # 验证安装
    log_info "验证依赖安装..."
    uv run python -c "import pandas, numpy; print('✅ 核心依赖安装成功')"
    uv run python -c "import PyInstaller; print('✅ PyInstaller 安装成功')" 2>/dev/null || {
        log_info "安装 PyInstaller..."
        uv add pyinstaller
    }

    log_success "UV 依赖安装完成"
}

# 使用传统方式安装 Python 依赖
install_python_deps_pip() {
    log_info "使用 pip 安装 Python 依赖..."
    pip3 install -r requirements.txt
    log_success "pip 依赖安装完成"
}

# 使用 UV 构建 Python 分析器
build_python_analyzer_uv() {
    log_info "使用 UV 构建 Python 分析器..."

    # 检查 Python 脚本是否存在
    if [ ! -f "src/bin/audio_analyzer.py" ]; then
        log_error "Python 分析器脚本不存在: src/bin/audio_analyzer.py"
        exit 1
    fi

    # 确保依赖已安装
    install_python_deps_uv

    # 使用 UV 运行 PyInstaller
    log_info "使用 UV + PyInstaller 构建可执行文件..."
    uv run pyinstaller --onefile \
                       --name audio-analyzer \
                       --clean \
                       --distpath assets/binaries \
                       src/bin/audio_analyzer.py

    # 验证构建结果
    if [ -f "assets/binaries/audio-analyzer" ]; then
        log_success "Python 分析器构建成功"
        chmod +x assets/binaries/audio-analyzer
    else
        log_error "Python 分析器构建失败"
        exit 1
    fi
}

# 构建 Python 分析器（智能选择方式）
build_python_analyzer() {
    if [ "$USE_UV" = true ] && [ "$FORCE_NO_UV" != true ]; then
        build_python_analyzer_uv
    else
        log_info "使用传统方式构建 Python 分析器..."

        # 检查 Python 脚本是否存在
        if [ ! -f "src/bin/audio_analyzer.py" ]; then
            log_error "Python 分析器脚本不存在: src/bin/audio_analyzer.py"
            exit 1
        fi

        # 安装 Python 依赖
        install_python_deps_pip

        # 使用 PyInstaller 构建
        log_info "使用 PyInstaller 构建可执行文件..."
        pyinstaller --onefile \
                    --name audio-analyzer \
                    --clean \
                    --distpath assets/binaries \
                    src/bin/audio_analyzer.py

        # 验证构建结果
        if [ -f "assets/binaries/audio-analyzer" ]; then
            log_success "Python 分析器构建成功"
            chmod +x assets/binaries/audio-analyzer
        else
            log_error "Python 分析器构建失败"
            exit 1
        fi
    fi
}

# 构建 Rust 主程序
build_rust_binary() {
    log_info "构建 Rust 主程序..."
    
    # 构建发布版本
    cargo build --release
    
    # 验证构建结果
    if [ -f "target/release/audio-analyzer" ]; then
        log_success "Rust 主程序构建成功"
    else
        log_error "Rust 主程序构建失败"
        exit 1
    fi
}

# 创建发布包
create_release_package() {
    local version=$(cargo metadata --format-version 1 | jq -r '.packages[] | select(.name == "audio_analyzer_ultimate") | .version')
    local package_name="audio-analyzer-v${version}-$(uname -m)-$(uname -s | tr '[:upper:]' '[:lower:]')"
    
    log_info "创建发布包: ${package_name}"
    
    # 创建发布目录
    mkdir -p "releases/${package_name}"
    
    # 复制主要文件
    cp target/release/audio-analyzer "releases/${package_name}/"
    cp README.md "releases/${package_name}/"
    cp -r docs/ "releases/${package_name}/"
    cp -r examples/ "releases/${package_name}/"
    
    # 创建安装脚本
    cat > "releases/${package_name}/install.sh" << 'EOF'
#!/bin/bash
# 音频质量分析器安装脚本

INSTALL_DIR="/usr/local/bin"

echo "正在安装音频质量分析器..."

# 检查权限
if [ "$EUID" -ne 0 ]; then
    echo "请使用 sudo 运行此脚本"
    exit 1
fi

# 复制可执行文件
cp audio-analyzer "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/audio-analyzer"

echo "安装完成！"
echo "使用 'audio-analyzer --help' 查看帮助信息"
EOF
    
    chmod +x "releases/${package_name}/install.sh"
    
    # 创建压缩包
    cd releases
    tar -czf "${package_name}.tar.gz" "${package_name}/"
    cd ..
    
    log_success "发布包创建完成: releases/${package_name}.tar.gz"
}

# 验证构建结果
verify_build() {
    log_info "验证构建结果..."
    
    # 检查主程序
    if [ -f "target/release/audio-analyzer" ]; then
        log_info "测试主程序..."
        ./target/release/audio-analyzer --help > /dev/null
        log_success "主程序验证通过"
    else
        log_error "主程序不存在"
        exit 1
    fi
    
    # 检查 Python 分析器
    if [ -f "assets/binaries/audio-analyzer" ]; then
        log_success "Python 分析器存在"
    else
        log_warning "Python 分析器不存在，某些功能可能不可用"
    fi
}

# 主函数
main() {
    echo "🎵 音频质量分析器构建脚本 v4.0"
    echo "=================================="
    
    # 解析命令行参数
    CLEAN=false
    WITH_BENCH=false
    SKIP_PYTHON=false
    CREATE_PACKAGE=false
    FORCE_USE_UV=false
    FORCE_NO_UV=false

    while [[ $# -gt 0 ]]; do
        case $1 in
            --clean)
                CLEAN=true
                shift
                ;;
            --with-bench)
                WITH_BENCH=true
                shift
                ;;
            --skip-python)
                SKIP_PYTHON=true
                shift
                ;;
            --package)
                CREATE_PACKAGE=true
                shift
                ;;
            --use-uv)
                FORCE_USE_UV=true
                shift
                ;;
            --no-uv)
                FORCE_NO_UV=true
                shift
                ;;
            --help)
                echo "用法: $0 [选项]"
                echo ""
                echo "选项:"
                echo "  --clean        清理构建目录"
                echo "  --with-bench   运行基准测试"
                echo "  --skip-python  跳过 Python 分析器构建"
                echo "  --package      创建发布包"
                echo "  --use-uv       强制使用 UV 工具（如果可用）"
                echo "  --no-uv        禁用 UV 工具，使用传统 pip 方式"
                echo "  --help         显示此帮助信息"
                echo ""
                echo "UV 工具说明:"
                echo "  UV 是一个极快的 Python 包管理器，可以显著提升构建速度。"
                echo "  如果系统中安装了 UV，默认会自动使用。"
                echo "  使用 --no-uv 可以强制使用传统的 pip 方式。"
                exit 0
                ;;
            *)
                log_error "未知选项: $1"
                exit 1
                ;;
        esac
    done

    # 处理 UV 相关的强制选项
    if [ "$FORCE_USE_UV" = true ] && [ "$FORCE_NO_UV" = true ]; then
        log_error "不能同时使用 --use-uv 和 --no-uv 选项"
        exit 1
    fi

    if [ "$FORCE_USE_UV" = true ]; then
        if ! command -v uv &> /dev/null; then
            log_error "指定了 --use-uv 但系统中未安装 UV 工具"
            log_info "请安装 UV: curl -LsSf https://astral.sh/uv/install.sh | sh"
            exit 1
        fi
        USE_UV=true
    fi
    
    # 执行构建步骤
    check_dependencies
    
    if [ "$CLEAN" = true ]; then
        clean_build
    fi
    
    if [ "$WITH_BENCH" = true ]; then
        run_tests --with-bench
    else
        run_tests
    fi
    
    if [ "$SKIP_PYTHON" = false ]; then
        build_python_analyzer
    fi
    
    build_rust_binary
    verify_build
    
    if [ "$CREATE_PACKAGE" = true ]; then
        create_release_package
    fi
    
    log_success "构建完成！"
    echo ""
    echo "可执行文件位置:"
    echo "  - Rust 主程序: target/release/audio-analyzer"
    if [ "$SKIP_PYTHON" = false ]; then
        echo "  - Python 分析器: assets/binaries/audio-analyzer"
    fi
    echo ""
    echo "使用 './target/release/audio-analyzer --help' 查看使用说明"
}

# 运行主函数
main "$@"
