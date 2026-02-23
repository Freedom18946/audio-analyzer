#!/usr/bin/env bash
# 一键构建/运行/打包脚本（面向下载仓库后的快速部署）

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

MODE="${1:-build}"
if [[ $# -gt 0 ]]; then
  shift
fi

WITH_TESTS=false
SKIP_PY_ANALYZER=false
FORCE=false
NO_SMOKE=false

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_ok() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_err() { echo -e "${RED}[ERR]${NC} $1"; }

usage() {
  cat <<'EOF'
用法:
  ./scripts/quickstart.sh [build|run|package] [选项]

模式:
  build      一键构建（默认）
  run        构建后直接运行 target/release/audio-analyzer
  package    构建并生成可分发压缩包（dist/*.tar.gz）

选项:
  --with-tests         构建前运行 Rust 测试
  --skip-py-analyzer   跳过 PyInstaller 打包 Python 分析器
  --force              强制重建（忽略缓存/时间戳）
  --no-smoke           跳过构建后冒烟验证
  -h, --help           显示帮助

示例:
  ./scripts/quickstart.sh
  ./scripts/quickstart.sh build --with-tests
  ./scripts/quickstart.sh run -- /path/to/music
  ./scripts/quickstart.sh package
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --with-tests)
      WITH_TESTS=true
      shift
      ;;
    --skip-py-analyzer)
      SKIP_PY_ANALYZER=true
      shift
      ;;
    --force)
      FORCE=true
      shift
      ;;
    --no-smoke)
      NO_SMOKE=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      break
      ;;
    *)
      break
      ;;
  esac
done

RUN_ARGS=("$@")

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    log_err "未找到命令: $cmd"
    exit 1
  fi
}

ensure_uv() {
  if command -v uv >/dev/null 2>&1; then
    log_ok "UV 已安装: $(uv --version)"
    return
  fi

  log_warn "未检测到 UV，尝试自动安装..."
  require_cmd curl
  curl -LsSf https://astral.sh/uv/install.sh | sh
  export PATH="$HOME/.cargo/bin:$PATH"

  if ! command -v uv >/dev/null 2>&1; then
    log_err "UV 自动安装失败，请手动安装后重试: https://docs.astral.sh/uv/"
    exit 1
  fi

  log_ok "UV 安装完成: $(uv --version)"
}

sync_python_deps() {
  ensure_uv
  if [[ -f "uv.lock" ]]; then
    log_info "使用锁文件同步 Python 依赖..."
    uv sync --frozen --no-dev
  else
    log_info "同步 Python 依赖..."
    uv sync --no-dev
  fi
}

build_python_analyzer() {
  if [[ "$SKIP_PY_ANALYZER" == true ]]; then
    log_warn "按参数跳过 Python 分析器打包"
    return
  fi

  local output="target/release/audio-analyzer-py"
  local src="src/bin/audio_analyzer.py"

  if [[ "$FORCE" == false && -f "$output" && "$src" -ot "$output" ]]; then
    log_info "Python 分析器已是最新，跳过打包"
    return
  fi

  sync_python_deps
  mkdir -p build/pyinstaller target/release

  log_info "打包 Python 分析器（PyInstaller）..."
  uv run pyinstaller \
    --onefile \
    --name audio-analyzer-py \
    --clean \
    --distpath target/release \
    --workpath build/pyinstaller \
    --specpath build/pyinstaller \
    src/bin/audio_analyzer.py

  chmod +x "$output"
  log_ok "Python 分析器已生成: $output"
}

build_rust_binary() {
  require_cmd cargo
  log_info "构建 Rust 主程序..."
  cargo build --release --locked
  log_ok "Rust 主程序构建完成: target/release/audio-analyzer"
}

run_tests_if_needed() {
  if [[ "$WITH_TESTS" == true ]]; then
    log_info "运行测试..."
    cargo test --lib --tests --bins
    log_ok "测试通过"
  fi
}

smoke_check() {
  if [[ "$NO_SMOKE" == true ]]; then
    log_warn "按参数跳过冒烟检查"
    return
  fi

  log_info "执行冒烟检查..."
  ./target/release/audio-analyzer --version >/dev/null
  if [[ -f "target/release/audio-analyzer-py" ]]; then
    ./target/release/audio-analyzer-py --help >/dev/null || true
  fi
  log_ok "冒烟检查完成"
}

build_all() {
  run_tests_if_needed
  build_python_analyzer
  build_rust_binary
  smoke_check
}

package_release() {
  build_all

  local version
  version="$(grep -m1 '^version = ' Cargo.toml | sed -E 's/version = "(.*)"/\1/')"
  local os
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  local arch
  arch="$(uname -m)"
  local pkg_name="audio-analyzer-v${version}-${arch}-${os}"
  local pkg_dir="dist/${pkg_name}"

  rm -rf "$pkg_dir"
  mkdir -p "$pkg_dir"

  cp target/release/audio-analyzer "$pkg_dir/"
  cp src/bin/audio_analyzer.py "$pkg_dir/"
  if [[ -f "target/release/audio-analyzer-py" ]]; then
    cp target/release/audio-analyzer-py "$pkg_dir/"
  fi
  if [[ -f "assets/binaries/ffmpeg" ]]; then
    cp assets/binaries/ffmpeg "$pkg_dir/"
  fi
  cp README.md "$pkg_dir/"

  cat > "$pkg_dir/run.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$SELF_DIR/audio-analyzer" "$@"
EOF
  chmod +x "$pkg_dir/run.sh"

  (cd dist && tar -czf "${pkg_name}.tar.gz" "${pkg_name}")
  log_ok "打包完成: dist/${pkg_name}.tar.gz"
}

case "$MODE" in
  build)
    build_all
    ;;
  run)
    build_all
    exec ./target/release/audio-analyzer "${RUN_ARGS[@]}"
    ;;
  package)
    package_release
    ;;
  *)
    log_err "未知模式: $MODE"
    usage
    exit 1
    ;;
esac

echo
log_ok "完成。可执行文件: ./target/release/audio-analyzer"
if [[ -f "target/release/audio-analyzer-py" ]]; then
  log_ok "已启用内置 Python 分析器: ./target/release/audio-analyzer-py"
fi
