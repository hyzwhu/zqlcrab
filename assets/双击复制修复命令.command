#!/bin/bash
CMD="sudo xattr -rd com.apple.quarantine /Applications/zqlcrab.app"

# 1. 复制到剪贴板 / Copy to clipboard
printf '%s' "$CMD" | pbcopy

# 2. 发送 macOS 系统通知 / Send macOS notification
osascript -e 'display notification "已将隔离修复命令复制到剪贴板！\nCommand copied to clipboard." with title "zqlcrab" subtitle "修复命令已复制 / Command Copied"' 2>/dev/null

# 3. 终端清晰双语输出 / Bilingual terminal banner
echo "================================================================"
echo "  zqlcrab macOS Gatekeeper 隔离属性修复 / Quarantine Fix"
echo "================================================================"
echo ""
echo "✅ 已成功将修复命令复制到剪贴板！"
echo "   Command has been copied to clipboard:"
echo ""
echo "   $CMD"
echo ""
echo "----------------------------------------------------------------"
echo "💡 使用指引 / Instructions:"
echo "   • 您可以直接打开「终端」并按 Cmd + V 粘贴该命令执行。"
echo "     You can open Terminal and press Cmd + V to paste and run."
echo ""
echo "   • 或者，您也可以直接在当前窗口一键执行修复（需输入 Mac 开机密码）："
echo "     Or, you can execute the fix directly in this window:"
echo "----------------------------------------------------------------"
echo ""
read -p "👉 是否现在直接执行修复？/ Run fix now? (y/N): " choice
if [[ "$choice" == [yY] || "$choice" == [yY][eE][sS] ]]; then
    echo ""
    if [ -d "/Applications/zqlcrab.app" ]; then
        echo "正在执行 / Running: $CMD"
        sudo xattr -rd com.apple.quarantine /Applications/zqlcrab.app
        if [ $? -eq 0 ]; then
            echo ""
            echo "🎉 修复成功！现在可以直接从「启动台」或「应用程序」打开 zqlcrab 了。"
            echo "   Success! You can now open zqlcrab from Applications or Launchpad."
        else
            echo ""
            echo "⚠️ 执行失败，请检查密码是否正确。"
            echo "   Execution failed. Please verify your password."
        fi
    else
        echo "⚠️ 未在 /Applications 目录下找到 zqlcrab.app！"
        echo "👉 请先将 zqlcrab 拖拽到 Applications 文件夹，再运行此修复。"
        echo "   Please drag zqlcrab.app into Applications folder first."
    fi
    echo ""
    read -p "按回车键退出... / Press Enter to exit..."
else
    echo ""
    echo "已跳过执行。命令仍在剪贴板中，随时可在终端中粘贴 (Cmd+V) 使用。"
    echo "Skipped. The command is in your clipboard ready to paste (Cmd+V)."
    sleep 2
fi
