# 极简番茄钟 · Minimal Pomodoro

<img src="src-tauri/icons/128x128.png" width="80" height="80" alt="番茄时钟图标">

中文、离线的 Windows 桌面番茄钟。专注学习，只记录今日完成次数和已完成专注分钟；无需账号，不发送遥测。

## 下载使用

到 [Releases](https://github.com/lumikok/minimal-pomodoro/releases/latest) 下载 `minimal-pomodoro_*_windows-x64_setup.exe` 并安装，然后从开始菜单打开「极简番茄钟」。普通用户无需安装 Rust、Node 或编译器。

适用于 Windows 10/11 **x64**。运行需要 Microsoft Edge WebView2；多数电脑已有，缺少时安装器会联网获取微软运行时。安装完成后番茄钟可离线使用。安装包未购买代码签名证书。Release 附有 `SHA256SUMS.txt`，用于校验下载完整性。

## 功能

- 25、50 分钟专注预设；专注及休息均可自定义 1–180 分钟。
- 开始、暂停、继续、重置；放弃未完成段需要确认。
- 每四次完整专注推荐长休息，下一阶段手动开始，可跳过休息。
- 到点一次通知和可关闭的提示音；不抢窗口焦点。
- 关闭窗口进入托盘，可选置顶；重复启动显示已有窗口。
- 休眠期间冻结倒计时；重启后未完成段恢复为暂停。
- 设置修改从下一段生效，完整完成的专注按本地完成日期统计。

正式数据位于 `%LOCALAPPDATA%\com.lumikok.minimal-pomodoro`，其中 `state.json` 保存设置和学习记录。更新安装包不会清空记录。系统「已安装的应用」可卸载程序；如不再需要记录，备份后可另行删除数据目录。

不提供任务管理、历史图表、账号、同步或自动更新。

## 源码与构建

Tauri 2 + TypeScript + Vite + Rust。开发依赖、缓存和测试数据使用项目内目录；C++ Build Tools/Windows SDK 属于系统前置依赖。

```powershell
.\scripts\install-build-tools.ps1 # 仅缺少微软 C++/SDK 时运行
.\scripts\project.ps1 setup
.\scripts\project.ps1 install
.\scripts\project.ps1 dev
.\scripts\project.ps1 check
.\scripts\project.ps1 test
.\scripts\project.ps1 build
```

图标使用自有生成脚本 `node scripts/generate-icons.mjs`，无需额外图像依赖。GitHub Actions 的 `Windows release build` 可手动构建并检查 Windows 安装包，不自动发布。

文档：[产品规则](docs/PRODUCT.md) · [技术设计](docs/TECHNICAL.md) · [版本记录](docs/ROADMAP.md) · [验收记录](docs/VALIDATION.md)。

## 许可

[MIT](LICENSE)。这是个人学习项目，当前停止功能开发；源码公开，欢迎自行使用或修改。