#!/bin/bash

# 音频质量分析器 - 发布前清理脚本 v4.0.0
# 清理所有构建产物、临时文件和缓存，为 GitHub 发布做准备

set -euo pipefail

# 兼容 macOS 默认的 bash 3.2

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

log_clean() {
    echo -e "${CYAN}[CLEAN]${NC} $1"
}

# 显示帮助信息
show_help() {
    cat << EOF
🧹 音频质量分析器发布前清理脚本 v4.0.0

用法: $0 [选项]

选项:
  -h, --help          显示此帮助信息
  -y, --yes           自动确认所有清理操作（非交互模式）
  -v, --verbose       显示详细的清理过程
  --dry-run          只显示将要清理的文件，不实际删除
  --keep-cache       保留缓存文件（.cache、.mypy_cache等）
  --keep-env         保留虚拟环境文件（.venv、uv.lock等）

示例:
  $0                  # 交互式清理
  $0 -y               # 自动清理
  $0 --dry-run        # 预览清理操作
  $0 -y --keep-cache  # 自动清理但保留缓存

清理内容:
  ✓ 构建产物 (target/, build/, dist/, assets/binaries/)
  ✓ 临时文件 (.DS_Store, *.pyc, __pycache__/)
  ✓ 环境文件 (.venv/, uv.lock, *.egg-info/)
  ✓ 缓存文件 (*.log, .cache/, .mypy_cache/)
  ✓ 测试产物 (.pytest_cache/, .coverage)

保留内容:
  ✓ 源代码和配置文件
  ✓ 文档和 README
  ✓ Git 配置和历史
  ✓ 许可证和版权文件
EOF
}

# 解析命令行参数
AUTO_CONFIRM=false
VERBOSE=false
DRY_RUN=false
KEEP_CACHE=false
KEEP_ENV=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_help
            exit 0
            ;;
        -y|--yes)
            AUTO_CONFIRM=true
            shift
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --keep-cache)
            KEEP_CACHE=true
            shift
            ;;
        --keep-env)
            KEEP_ENV=true
            shift
            ;;
        *)
            log_error "未知选项: $1"
            echo "使用 $0 --help 查看帮助信息"
            exit 1
            ;;
    esac
done

# 检查是否在项目根目录
if [[ ! -f "Cargo.toml" ]] || [[ ! -f "pyproject.toml" ]]; then
    log_error "请在项目根目录运行此脚本"
    exit 1
fi

# 显示脚本头部信息
echo -e "${PURPLE}🧹 音频质量分析器发布前清理脚本 v4.0.0${NC}"
echo "=============================================="

if [[ "$DRY_RUN" == true ]]; then
    log_warning "运行在预览模式，不会实际删除文件"
fi

# 定义清理目标
define_clean_targets() {
    # 基础清理目标（保留重要的二进制依赖文件）
    BUILD_ARTIFACTS="target/ build/ dist/ releases/"
    # 只清理 assets/binaries/ 中的构建产物，保留 ffmpeg 等依赖文件
    BINARY_ARTIFACTS="assets/binaries/audio-analyzer"
    PYTHON_CACHE="__pycache__/ *.pyc *.pyo *.pyd .pytest_cache/ .coverage htmlcov/"
    SYSTEM_TEMP=".DS_Store .DS_Store? ._* .Spotlight-V100 .Trashes ehthumbs.db Thumbs.db"
    EDITOR_TEMP="*.swp *.swo *~ .vscode/.ropeproject .idea/"
    LOG_FILES="*.log *.log.* npm-debug.log* yarn-debug.log* yarn-error.log*"

    # 可选清理目标
    if [[ "$KEEP_ENV" != true ]]; then
        ENV_FILES=".venv/ uv.lock *.egg-info/ .tox/ .nox/"
    else
        ENV_FILES=""
    fi

    if [[ "$KEEP_CACHE" != true ]]; then
        CACHE_FILES=".cache/ .mypy_cache/ .ruff_cache/ .uv-cache/ node_modules/"
    else
        CACHE_FILES=""
    fi
}

# 统计函数
count_files() {
    local pattern="$1"
    local count=0
    
    for item in $pattern; do
        if [[ -e "$item" ]]; then
            if [[ -d "$item" ]]; then
                count=$((count + $(find "$item" -type f 2>/dev/null | wc -l)))
            else
                count=$((count + $(find . -maxdepth 1 -name "$item" -type f 2>/dev/null | wc -l)))
            fi
        fi
    done
    
    echo $count
}

# 清理函数
clean_pattern() {
    local pattern="$1"
    local description="$2"
    local removed_count=0
    
    for item in $pattern; do
        if [[ -e "$item" ]]; then
            if [[ "$VERBOSE" == true ]]; then
                log_clean "删除: $item"
            fi
            
            if [[ "$DRY_RUN" != true ]]; then
                rm -rf "$item"
            fi
            
            if [[ -d "$item" ]]; then
                removed_count=$((removed_count + 1))
            else
                removed_count=$((removed_count + $(find . -maxdepth 1 -name "$item" 2>/dev/null | wc -l)))
            fi
        fi
    done
    
    if [[ $removed_count -gt 0 ]]; then
        log_success "$description: 清理了 $removed_count 个项目"
    else
        log_info "$description: 无需清理"
    fi
}

# 显示清理预览
show_preview() {
    log_step "扫描需要清理的文件..."
    echo

    local total_files=0

    # 检查各类文件
    check_and_show() {
        local name="$1"
        local pattern="$2"

        if [[ -n "$pattern" ]]; then
            local count=$(count_files "$pattern")

            if [[ $count -gt 0 ]]; then
                echo -e "${YELLOW}📁 $name${NC}: $count 个文件/目录"
                total_files=$((total_files + count))

                if [[ "$VERBOSE" == true ]]; then
                    for item in $pattern; do
                        if [[ -e "$item" ]]; then
                            echo "   - $item"
                        fi
                    done
                fi
            else
                echo -e "${GREEN}📁 $name${NC}: 无需清理"
            fi
        fi
    }

    check_and_show "构建产物" "$BUILD_ARTIFACTS"
    check_and_show "二进制产物" "$BINARY_ARTIFACTS"
    check_and_show "Python缓存" "$PYTHON_CACHE"
    check_and_show "系统临时文件" "$SYSTEM_TEMP"
    check_and_show "编辑器临时文件" "$EDITOR_TEMP"
    check_and_show "日志文件" "$LOG_FILES"
    check_and_show "环境文件" "$ENV_FILES"
    check_and_show "缓存文件" "$CACHE_FILES"

    echo
    echo -e "${CYAN}总计: $total_files 个文件/目录将被清理${NC}"
    echo
}

# 确认清理操作
confirm_cleanup() {
    if [[ "$AUTO_CONFIRM" == true ]]; then
        return 0
    fi
    
    echo -e "${YELLOW}⚠️  警告: 此操作将永久删除上述文件和目录${NC}"
    echo -e "${YELLOW}   请确保您已经提交了所有重要的更改到 Git${NC}"
    echo
    
    read -p "是否继续清理? [y/N]: " -n 1 -r
    echo
    
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log_info "清理操作已取消"
        exit 0
    fi
}

# 主清理流程
main() {
    # 定义清理目标
    define_clean_targets

    # 显示清理预览
    show_preview

    # 确认清理操作
    if [[ "$DRY_RUN" != true ]]; then
        confirm_cleanup
    fi

    # 执行清理
    log_step "开始清理..."
    echo

    # 执行各类清理
    clean_category() {
        local name="$1"
        local pattern="$2"

        if [[ -n "$pattern" ]]; then
            clean_pattern "$pattern" "$name"
        fi
    }

    clean_category "构建产物" "$BUILD_ARTIFACTS"
    clean_category "二进制产物" "$BINARY_ARTIFACTS"
    clean_category "Python缓存" "$PYTHON_CACHE"
    clean_category "系统临时文件" "$SYSTEM_TEMP"
    clean_category "编辑器临时文件" "$EDITOR_TEMP"
    clean_category "日志文件" "$LOG_FILES"
    clean_category "环境文件" "$ENV_FILES"
    clean_category "缓存文件" "$CACHE_FILES"
    
    echo
    
    if [[ "$DRY_RUN" == true ]]; then
        log_success "预览完成！使用 $0 -y 执行实际清理"
    else
        log_success "🎉 清理完成！项目已准备好进行发布"
        echo
        log_info "下一步操作:"
        echo "  1. 验证项目仍可正常构建: ./scripts/deploy-uv.sh"
        echo "  2. 运行测试确保功能正常: cargo test"
        echo "  3. 创建发布包: ./scripts/build.sh --package"
        echo "  4. 创建 GitHub Release"
    fi
}

# 执行主函数
main
