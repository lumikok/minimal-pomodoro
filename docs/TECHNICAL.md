# 技术设计

## 技术决策

采用 Tauri 2 + 原生 TypeScript + Vite + Rust，npm 管理前端依赖。不引入前端框架、服务端或数据库。前端只渲染 Rust 的计时快照，不自行递减时间。

官方依据：

- [Tauri 环境要求](https://v2.tauri.app/start/prerequisites/)
- [Rust 局部安装](https://github.com/rust-lang/rustup/blob/main/doc/user-guide/src/installation/index.md)
- [Tauri 配置与 useLocalToolsDir](https://v2.tauri.app/reference/config/)
- [Windows 排除休眠的时钟](https://learn.microsoft.com/en-us/windows/win32/api/realtimeapiset/nf-realtimeapiset-queryunbiasedinterrupttime)
- [Tauri 通知](https://v2.tauri.app/plugin/notification/)
- [Windows 安装包](https://v2.tauri.app/distribute/windows-installer/)

## 项目环境

| 内容 | 路径 |
| --- | --- |
| Rust 管理器与工具链 | `.tools/rustup` |
| Cargo 工具与依赖 | `.tools/cargo` |
| npm 依赖 | `node_modules` |
| npm 缓存、临时文件 | `.cache/npm`、`.cache/tmp` |
| Rust 编译与打包缓存 | `.cache/target` |
| 开发数据与 WebView 数据 | `.data/dev` |

启动脚本只为子进程设置 RUSTUP_HOME、CARGO_HOME、CARGO_TARGET_DIR、npm 缓存、TEMP/TMP、局部 PATH。不修改注册表环境变量或全局 npm 配置。rustup 安装使用 `--no-modify-path`；Tauri 启用 `bundle.useLocalToolsDir`。已有 Node/Git/WebView2 直接复用。

脚本执行期间临时设置当前启动进程的环境供子进程继承，`finally` 精确恢复原值（原来未定义的变量删除，而不是留空字符串）。`scripts/verify-environment.ps1` 对 Process/User/Machine 三个作用域逐项比较，已实际通过。

微软 C++ Build Tools 和 Windows SDK 允许系统安装，先检查已有组件，只补齐缺失组件。安装器及部分共享组件会写入系统目录，这是环境局部化的明确例外。开发用数据路径由启动脚本设置；发布版不继承开发路径。

## 架构与接口

- Rust 核心：阶段、状态、剩余时间、设置、今日汇总、四轮循环；纯状态转换，可注入时钟测试。
- 桌面适配：计时工作线程、Tauri 命令与事件、托盘、原生 Windows Toast、系统短声音、置顶、单实例。
- 存储适配：带版本 JSON，状态变更与每五秒快照；原子替换，损坏文件保留。
- 前端：倒计时显示、阶段选择、输入验证、确认弹窗、设置与统计；所有动作等待 Rust 回应。

接口提供 `get_state`、`start_timer`、`pause_timer`、`resume_timer`、`reset_timer`、`switch_phase`、`save_settings`、置顶与首次托盘提示确认。命令返回统一快照；`timer-state` 事件同步后台变化。快照包括阶段、状态、剩余秒数、当前总时长、推荐下一阶段、设置、今日汇总与存储提示。

## 时间与持久化

Windows 使用 `QueryUnbiasedInterruptTime`，排除睡眠/休眠，不依赖系统墙上时钟。墙上时钟仅用于本地日期与错误备份文件名。剩余时间按单调时钟差计算，显示向上取整秒，避免频繁调度导致漂移。

状态转换和完成处理在同一互斥状态下执行，完成事件只产生一次。每个新专注段绑定开始时的时长，后续设置不影响该段。

Windows 通知使用 `tauri-winrt-notification` 原生封装，检查实际发送错误。通用通知插件在 Windows 后台忽略发送错误且 silent 选项不适用于桌面，因此最终实现不保留该插件。Toast 始终静音，独立 MessageBeep 仅在声音开关打开时调用一次；通知错误不会回滚完成统计。

事件快照带递增 revision，界面忽略过期事件/命令响应。初次关闭窗口通过应用内提示确认托盘行为；单实例第二次启动仅显示已有窗口。

保存记录包括 schema 版本、阶段快照、设置、当日汇总与循环次数。恢复运行中记录时转为暂停，以保存的剩余时间继续。不恢复单调时钟锚点，不补计离线时间。数据字段做范围及一致性验证；无效数据另存保留，应用以默认设置启动并告知用户。

## 构建与发布

目标 Windows 10/11 x64，NSIS 当前用户安装包，不启用自动更新或遥测。依赖锁文件提交；工具链、下载缓存、开发数据、编译与安装产物不提交。

开发模式只能验证通知调用流程，正式通知必须在安装版验收。未完成真实安装验收前不打 `v0.1.0` 标签。



## v0.1.1 发布构建

最终图标更新通过 GitHub Windows runner 完整构建，避免在本机重新安装编译器。Windows 工作流手动触发，测试及打包产物验收后由维护者发布。图标由无额外依赖的自有脚本生成，应用与 NSIS 使用同一 ICO。停止开发后的本机依赖清理不影响已安装应用。
