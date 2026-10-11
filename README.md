# 极简番茄钟 · Minimal Pomodoro

<img src="src-tauri/icons/128x128.png" width="80" height="80" alt="番茄时钟图标">

中文、离线的 Windows 桌面番茄钟。透明小浮窗常驻显示倒计时，点击展开设置和累计统计；无需账号，不发送遥测。

## 下载使用

到 [Releases](https://github.com/lumikok/minimal-pomodoro/releases/latest) 下载 `minimal-pomodoro_*_windows-x64_setup.exe` 并安装，然后从开始菜单打开「极简番茄钟」。普通用户无需安装 Rust、Node 或编译器。

适用于 Windows 10/11 **x64**。运行需要 Microsoft Edge WebView2；多数电脑已有，缺少时安装器会联网获取微软运行时。安装完成后番茄钟可离线使用。安装包未购买代码签名证书。Release 附有 `SHA256SUMS.txt`，用于校验下载完整性。

## 功能

- 25、50 分钟专注预设；专注及休息均可自定义 1–180 分钟。
- 开始、暂停、继续、重置；放弃未完成段需要确认。
- 每四次完整专注推荐长休息，下一阶段手动开始，可跳过休息。
- 约 220×100 的透明浮窗，拖动时间区域移动；默认置顶，底色不透明度默认 80%，可调整。
- 点击右侧展开按钮查看设置和统计；顶部可收起面板或隐藏到托盘。
- 到点自动显示持续的置顶完成卡片，托盘隐藏时也会出现；约 10 秒铃声可关闭，不抢键盘焦点。
- 累计完成次数和专注时长，支持「今日」切换；时长满 60 分钟显示小时，倒计时满一小时显示时、分、秒。
- 关闭窗口进入托盘，可取消置顶；重复启动显示已有窗口。
- 休眠期间冻结倒计时；重启后未完成段恢复为暂停。
- 设置修改从下一段生效，完整完成的专注按本地完成日期统计。

正式数据位于 `%LOCALAPPDATA%\com.lumikok.minimal-pomodoro`，其中 `state.json` 保存设置和学习记录。更新安装包不会清空记录。系统「已安装的应用」可卸载程序；如不再需要记录，备份后可另行删除数据目录。

升级 v0.2 时会先保留 `state.pre-v0.2-*.json` 原始备份，再将旧版仍保留的那一天纳入累计。旧版已经覆盖的往日统计无法自动恢复，可在面板「补录旧版未保留的历史」填写缺失部分；再次保存会替换补录值，不重复叠加。累计栏标明现存记录起点。

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

图标和铃声由自有脚本生成（`scripts/generate-icons.mjs`、`scripts/generate-chime.mjs`），无需额外素材或生成依赖。GitHub Actions 的 `Windows release build` 可手动测试并构建 Windows 安装包；同时生成使用独立数据目录的验收程序，不自动发布。时间格式测试使用 Node 22 的 `--experimental-strip-types` 或 Node 24 运行 `tests/format.test.mjs`。

文档：[产品规则](docs/PRODUCT.md) · [技术设计](docs/TECHNICAL.md) · [版本记录](docs/ROADMAP.md) · [验收记录](docs/VALIDATION.md)。

## 许可

[MIT](LICENSE)。这是个人学习项目；源码公开，欢迎自行使用或修改。
