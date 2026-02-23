#!/bin/bash
# 音频质量分析器 UV 快捷部署脚本
# 
# 此脚本专门用于使用 UV 工具进行快速部署和构建
# 支持自动安装 UV、环境管理、依赖解析和一键部署

set -e  # 遇到错误时退出

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
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

log_step() {
    echo -e "${PURPLE}[STEP]${NC} $1"
}

log_performance() {
    echo -e "${CYAN}[PERF]${NC} $1"
}

# 检测操作系统
detect_os() {
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        OS="linux"
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        OS="macos"
    elif [[ "$OSTYPE" == "cygwin" ]] || [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]]; then
        OS="windows"
    else
        OS="unknown"
    fi
    log_info "检测到操作系统: $OS"
}

# 检查 UV 是否已安装
check_uv_installed() {
    if command -v uv &> /dev/null; then
        UV_VERSION=$(uv --version | cut -d' ' -f2)
        log_success "UV 已安装，版本: $UV_VERSION"
        return 0
    else
        log_warning "UV 未安装"
        return 1
    fi
}

# 自动安装 UV
install_uv() {
    log_step "正在安装 UV 工具..."
    
    case $OS in
        "linux"|"macos")
            log_info "使用官方安装脚本安装 UV..."
            curl -LsSf https://astral.sh/uv/install.sh | sh
            
            # 添加到当前会话的 PATH
            export PATH="$HOME/.cargo/bin:$PATH"
            
            # 验证安装
            if command -v uv &> /dev/null; then
                UV_VERSION=$(uv --version | cut -d' ' -f2)
                log_success "UV 安装成功，版本: $UV_VERSION"
            else
                log_error "UV 安装失败"
                exit 1
            fi
            ;;
        "windows")
            log_info "Windows 系统请手动安装 UV:"
            log_info "PowerShell: powershell -c \"irm https://astral.sh/uv/install.ps1 | iex\""
            log_info "或者使用 pip: pip install uv"
            exit 1
            ;;
        *)
            log_error "不支持的操作系统: $OS"
            log_info "请手动安装 UV: pip install uv"
            exit 1
            ;;
    esac
}

# 性能计时器
start_timer() {
    TIMER_START=$(date +%s.%N)
}

end_timer() {
    TIMER_END=$(date +%s.%N)
    DURATION=$(echo "$TIMER_END - $TIMER_START" | bc -l 2>/dev/null || echo "0")
    log_performance "耗时: ${DURATION}s"
}

# 创建和配置虚拟环境
setup_venv() {
    log_step "设置 UV 虚拟环境..."
    
    start_timer
    
    # 创建虚拟环境（如果不存在）
    if [ ! -d ".venv" ]; then
        log_info "创建新的虚拟环境..."
        uv venv --python 3.8
    else
        log_info "使用现有虚拟环境..."
    fi
    
    # 显示虚拟环境信息
    log_info "虚拟环境路径: $(pwd)/.venv"
    
    end_timer
    log_success "虚拟环境设置完成"
}

# 快速依赖安装
install_dependencies() {
    log_step "安装项目依赖..."

    start_timer

    # 直接安装核心依赖，不使用项目构建
    log_info "安装核心依赖..."
    uv pip install pandas numpy pyinstaller

    # 安装开发依赖（可选）
    if [ "$SKIP_QUALITY" != true ]; then
        log_info "安装开发依赖..."
        uv pip install pytest black flake8 mypy isort
    fi

    end_timer
    log_success "依赖安装完成"

    # 验证关键依赖
    log_info "验证关键依赖..."
    uv run python -c "
import sys
print(f'Python 版本: {sys.version}')

try:
    import pandas as pd
    print(f'✅ pandas {pd.__version__}')
except ImportError as e:
    print(f'❌ pandas: {e}')

try:
    import numpy as np
    print(f'✅ numpy {np.__version__}')
except ImportError as e:
    print(f'❌ numpy: {e}')

try:
    import PyInstaller
    print(f'✅ PyInstaller {PyInstaller.__version__}')
except ImportError as e:
    print(f'❌ PyInstaller: {e}')
"
}

# 运行代码质量检查
run_quality_checks() {
    log_step "运行代码质量检查..."
    
    if [ "$SKIP_QUALITY" != true ]; then
        log_info "运行代码格式化检查..."
        uv run black --check src/ || {
            log_warning "代码格式不符合标准，正在自动格式化..."
            uv run black src/
        }
        
        log_info "运行导入排序检查..."
        uv run isort --check-only src/ || {
            log_warning "导入顺序不符合标准，正在自动修复..."
            uv run isort src/
        }
        
        log_info "运行类型检查..."
        uv run mypy src/ || log_warning "类型检查发现问题，请检查代码"
        
        log_info "运行代码风格检查..."
        uv run flake8 src/ || log_warning "代码风格检查发现问题，请检查代码"
        
        log_success "代码质量检查完成"
    else
        log_info "跳过代码质量检查"
    fi
}

# 构建 Python 分析器
build_analyzer() {
    log_step "构建 Python 分析器..."
    
    # 检查源文件
    if [ ! -f "src/bin/audio_analyzer.py" ]; then
        log_error "Python 分析器脚本不存在: src/bin/audio_analyzer.py"
        exit 1
    fi
    
    # 确保输出目录存在
    mkdir -p assets/binaries
    
    start_timer
    
    # 使用 UV 运行 PyInstaller
    log_info "使用 UV + PyInstaller 构建可执行文件..."
    uv run pyinstaller \
        --onefile \
        --name audio-analyzer \
        --clean \
        --distpath assets/binaries \
        --workpath build \
        --specpath build \
        src/bin/audio_analyzer.py
    
    end_timer
    
    # 验证构建结果
    if [ -f "assets/binaries/audio-analyzer" ]; then
        chmod +x assets/binaries/audio-analyzer
        BINARY_SIZE=$(du -h assets/binaries/audio-analyzer | cut -f1)
        log_success "Python 分析器构建成功"
        log_info "可执行文件大小: $BINARY_SIZE"
        log_info "可执行文件路径: assets/binaries/audio-analyzer"
    else
        log_error "Python 分析器构建失败"
        exit 1
    fi
}

# 运行测试
run_tests() {
    log_step "运行测试套件..."
    
    if [ "$SKIP_TESTS" != true ]; then
        # Python 测试
        if [ -d "tests" ]; then
            log_info "运行 Python 测试..."
            uv run pytest tests/ -v || log_warning "Python 测试失败"
        fi
        
        # Rust 测试
        log_info "运行 Rust 测试..."
        cargo test --release || log_warning "Rust 测试失败"
        
        log_success "测试完成"
    else
        log_info "跳过测试"
    fi
}

# 环境验证和健康检查
health_check() {
    log_step "执行健康检查..."
    
    # 检查 Rust 构建
    log_info "验证 Rust 主程序..."
    if cargo build --release; then
        log_success "Rust 主程序构建成功"
        
        # 测试主程序
        if ./target/release/audio-analyzer --help > /dev/null 2>&1; then
            log_success "Rust 主程序运行正常"
        else
            log_warning "Rust 主程序运行异常"
        fi
    else
        log_error "Rust 主程序构建失败"
        exit 1
    fi
    
    # 检查 Python 分析器
    if [ -f "assets/binaries/audio-analyzer" ]; then
        log_success "Python 分析器存在"
        
        # 测试 Python 分析器
        if ./assets/binaries/audio-analyzer --help > /dev/null 2>&1; then
            log_success "Python 分析器运行正常"
        else
            log_warning "Python 分析器运行异常"
        fi
    else
        log_warning "Python 分析器不存在"
    fi
    
    log_success "健康检查完成"
}

# 清理构建产物
cleanup() {
    log_step "清理构建产物..."
    
    # 清理 Python 构建产物
    rm -rf build/ dist/
    find . -name "*.pyc" -delete
    find . -name "__pycache__" -type d -exec rm -rf {} + 2>/dev/null || true
    
    # 清理 Rust 构建产物
    cargo clean
    
    # 清理 UV 缓存（可选）
    if [ "$CLEAN_CACHE" = true ]; then
        uv cache clean
        log_info "已清理 UV 缓存"
    fi
    
    log_success "清理完成"
}

# 显示性能统计
show_performance_stats() {
    log_step "性能统计信息"
    
    # UV 缓存信息
    if command -v uv &> /dev/null; then
        log_info "UV 缓存信息:"
        uv cache info 2>/dev/null || log_warning "无法获取 UV 缓存信息"
    fi
    
    # 构建产物大小
    if [ -f "target/release/audio-analyzer" ]; then
        RUST_SIZE=$(du -h target/release/audio-analyzer | cut -f1)
        log_info "Rust 主程序大小: $RUST_SIZE"
    fi
    
    if [ -f "assets/binaries/audio-analyzer" ]; then
        PYTHON_SIZE=$(du -h assets/binaries/audio-analyzer | cut -f1)
        log_info "Python 分析器大小: $PYTHON_SIZE"
    fi
}

# 主函数
main() {
    echo "🚀 音频质量分析器 UV 快捷部署脚本 v4.0.1"
    echo "=============================================="
    
    # 解析命令行参数
    SKIP_QUALITY=false
    SKIP_TESTS=false
    CLEAN_FIRST=false
    CLEAN_CACHE=false
    FORCE_INSTALL_UV=false
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            --skip-quality)
                SKIP_QUALITY=true
                shift
                ;;
            --skip-tests)
                SKIP_TESTS=true
                shift
                ;;
            --clean)
                CLEAN_FIRST=true
                shift
                ;;
            --clean-cache)
                CLEAN_CACHE=true
                shift
                ;;
            --install-uv)
                FORCE_INSTALL_UV=true
                shift
                ;;
            --help)
                echo "用法: $0 [选项]"
                echo ""
                echo "选项:"
                echo "  --skip-quality  跳过代码质量检查"
                echo "  --skip-tests    跳过测试运行"
                echo "  --clean         开始前清理构建产物"
                echo "  --clean-cache   清理 UV 缓存"
                echo "  --install-uv    强制安装 UV（即使已存在）"
                echo "  --help          显示此帮助信息"
                echo ""
                echo "此脚本将自动:"
                echo "  1. 检测并安装 UV 工具"
                echo "  2. 创建和配置虚拟环境"
                echo "  3. 快速安装项目依赖"
                echo "  4. 运行代码质量检查"
                echo "  5. 构建 Python 分析器"
                echo "  6. 运行测试套件"
                echo "  7. 执行健康检查"
                exit 0
                ;;
            *)
                log_error "未知选项: $1"
                exit 1
                ;;
        esac
    done
    
    # 执行部署步骤
    detect_os
    
    if [ "$CLEAN_FIRST" = true ]; then
        cleanup
    fi
    
    # 检查或安装 UV
    if ! check_uv_installed || [ "$FORCE_INSTALL_UV" = true ]; then
        install_uv
    fi
    
    setup_venv
    install_dependencies
    run_quality_checks
    build_analyzer
    run_tests
    health_check
    show_performance_stats
    
    log_success "🎉 UV 快捷部署完成！"
    echo ""
    echo "📁 构建产物:"
    echo "  - Rust 主程序: target/release/audio-analyzer"
    echo "  - Python 分析器: assets/binaries/audio-analyzer"
    echo ""
    echo "🚀 使用方法:"
    echo "  ./target/release/audio-analyzer --help"
    echo ""
    echo "⚡ 性能提示:"
    echo "  UV 工具显著提升了 Python 依赖安装速度"
    echo "  后续构建将更加快速，因为依赖已缓存"
}

# 运行主函数
main "$@"
