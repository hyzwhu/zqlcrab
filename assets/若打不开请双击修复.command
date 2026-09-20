#!/bin/bash
echo "=========================================================="
echo "  zqlcrab macOS Gatekeeper 隔离属性修复工具"
echo "=========================================================="
echo ""
echo "正在为 /Applications/zqlcrab.app 移除隔离属性 (com.apple.quarantine)..."
echo "如提示 Password，请输入您的 Mac 开机登录密码并回车（密码输入时不会显示）："
echo ""
if [ -d "/Applications/zqlcrab.app" ]; then
    sudo xattr -rd com.apple.quarantine /Applications/zqlcrab.app 2>/dev/null || xattr -rd com.apple.quarantine /Applications/zqlcrab.app
    if [ $? -eq 0 ]; then
        echo ""
        echo "✅ 修复成功！现在您可以直接从「启动台」或「应用程序」打开 zqlcrab 了。"
    else
        echo ""
        echo "⚠️ 修复执行失败，请检查密码是否输入正确。"
    fi
else
    echo "⚠️ 未在 /Applications 目录下找到 zqlcrab.app！"
    echo "👉 请先将 zqlcrab 图标拖拽到 Applications (应用程序) 文件夹中，再运行本脚本修复。"
fi
echo ""
read -p "按回车键退出..."
