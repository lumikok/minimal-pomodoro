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

- Rust 核心：阶段、状态、剩余时间、设置、今日和累计汇总、历史补录、四轮循环与提醒确认；纯状态转换，可注入时钟测试。
- 桌面适配：计时工作线程、一个透明无标题栏窗口、托盘、原生 Windows Toast、内嵌铃声、置顶、位置恢复、单实例。
- 存储适配：带版本 JSON，状态变更与每五秒快照；原子替换，损坏文件保留。
- 前端：倒计时显示、阶段选择、输入验证、确认弹窗、设置与统计；所有动作等待 Rust 回应。

接口提供 `get_state`、`start_timer`、`pause_timer`、`resume_timer`、`reset_timer`、`switch_phase`、`save_settings`、`set_always_on_top`；v0.2 新增 `set_expanded`、`acknowledge_completion`、`silence_chime`、`save_history`、`save_opacity`、`hide_window`。命令返回统一快照；`timer-state` 事件同步后台变化。快照增加累计汇总、可替换历史补录、窗口偏好、提醒待确认、展开和正在响铃状态。前端格式函数只处理显示，不构建另一套计时器。

## 时间与持久化

Windows 使用 `QueryUnbiasedInterruptTime`，排除睡眠/休眠，不依赖系统墙上时钟。墙上时钟仅用于本地日期与错误备份文件名。剩余时间按单调时钟差计算，显示向上取整秒，避免频繁调度导致漂移。

状态转换和完成处理在同一互斥状态下执行，完成事件只产生一次。每个新专注段绑定开始时的时长，后续设置不影响该段。

Windows 通知使用 `tauri-winrt-notification` 原生封装，检查实际发送错误。Toast 始终静音；v0.2 使用 `PlaySoundW` 循环播放程序内嵌的原创 WAV，单调截止时间约 10 秒后停止，也可手动静音或确认停止。完成时通过 `ShowWindow(SW_SHOWNOACTIVATE)` 和 `SetWindowPos(...SWP_NOACTIVATE)` 显示置顶卡片，不调用焦点接口；显式从托盘或重复启动打开时才请求焦点。通知错误不会回滚完成统计。

事件快照带递增 revision，界面忽略过期事件/命令响应。面板关闭按钮明确标注隐藏至托盘；单实例第二次启动仅显示已有窗口。展开／收起共用一个 WebView，Rust 后台线程每 500 ms 检查计时。

schema v2 保存阶段快照、设置、当日汇总、累计 `lifetime`（起点、次数、分钟、独立 `history` 补录）、循环次数、`window`（40–100% 底色不透明度、物理坐标）和 `reminderPending`。累计只在完整专注完成时增加，跨日仅重建 today；补录通过赋值替换。

加载 v1 时先逐字节复制到同目录 `state.pre-v0.2-时间戳.json`，再迁移现存当日统计，最后处理换日，避免旧记录先被清空。备份失败时仅在内存恢复，禁止覆盖原文件并提示；启动即保存成功迁移的 v2。损坏文件保留，范围和一致性不通过则提示并使用默认数据。恢复运行中记录为暂停，不补计离线时间。待确认提醒跨重启保留，声音截止时间不持久化。降级 v0.1.x 前须退出应用并用迁移前备份替换 state.json，旧版不能读取 v2。

移动结束后约一秒保存位置，忽略程序布局产生的中间移动事件。每五秒复查显示器工作区，将浮窗完整限制在可见区域；尺寸按逻辑像素、位置按物理像素计算，结合对应显示器缩放。展开时临时夹紧坐标，收起时回到保存的浮窗锚点。

## 构建与发布

目标 Windows 10/11 x64，NSIS 当前用户安装包，不启用自动更新或遥测。依赖锁文件提交；工具链、下载缓存、开发数据、编译与安装产物不提交。

开发模式只能验证通知调用流程，正式通知必须在安装版验收。未完成真实安装验收前不打 `v0.1.0` 标签。

验证证据见 [VALIDATION.md](VALIDATION.md)。构建脚本另外将安装包复制到独立 `release`，可清理整个编译缓存而不丢交付物。


## v0.1.1 发布构建

最终图标更新通过 GitHub Windows runner 完整构建，避免在本机重新安装编译器。Windows 工作流手动触发，测试及打包产物验收后由维护者发布。图标由无额外依赖的自有脚本生成，应用与 NSIS 使用同一 ICO。停止开发后的本机依赖清理不影响已安装应用。

## v0.2 构建与隔离验收

继续在 GitHub Windows runner 运行 Rust 测试、前端格式测试、类型检查和 NSIS 打包。本机仅临时安装项目内 npm 依赖，不重新安装 Rust、C++ 或 Windows SDK，验收后清理。

工作流先保存正式主程序和安装包，再用 `acceptance` Cargo feature 与 `tests/acceptance-config.json` 构建独立验收程序。它使用不同应用标识，将状态与 WebView 数据写入 `POMODORO_DEV_DATA_DIR` 指定的项目测试目录。官方 Release 不启用该 feature，始终使用正式用户数据目录；验收程序不作为公开下载资产。

透明窗口配置 `transparent`、`decorations: false`、`skipTaskbar`、`noRedirectionBitmap`，仅授予必要拖动权限。扩缩和提醒都由 Rust 驱动，展开状态不写入数据，浮窗为默认启动形态。
