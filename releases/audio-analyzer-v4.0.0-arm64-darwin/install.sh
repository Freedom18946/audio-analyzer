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
