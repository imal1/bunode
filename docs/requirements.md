# Bunode — macOS 原生 Node.js / Bun 开发服务管理器需求说明

> 术语以 [`GLOSSARY.md`](../GLOSSARY.md) 为准：**Project**（项目目录）→ **Service**（可启停的运行配置）→ **Run**（一次实际运行）。关键架构决策见 [`docs/adr/`](adr/)。

## 1. 项目概述

### 1.1 产品定位

Bunode 是一款专为 macOS 设计的轻量级 Node.js / Bun 原生图形化进程管理工具。

面向日常使用 Node.js、Bun、npm、pnpm 等工具的开发者，统一管理本地运行的开发服务器、API 服务、Bot、MCP Server 及其他 JavaScript/TypeScript 后台进程。

产品**以托管为主**：主界面围绕用户在 Bunode 中配置的 Service；自动发现的外部进程是辅助视图，仅提供查看与安全终止。

产品定位类似 Quay，但更专注 Node.js / Bun 生态，强调：

- 简洁、美观、符合 macOS 设计规范（原生控件，不使用 WebView）。
- 稳定可靠，不出现 GUI 卡死、进程无法终止等问题。
- 快速启动、停止、重启和监控多个 Service。
- 自动发现运行中的 Node/Bun 进程。
- 支持实时日志、资源监控和端口查看。
- 轻量、低内存占用、低 CPU 占用。
- 本地运行，无需浏览器、云服务或账户。
- 开源免费，不设置功能付费墙。

### 1.2 目标平台

|项目|要求|
|---|---|
|操作系统|macOS 26 Tahoe 及以上|
|CPU 架构|Apple Silicon 优先，兼容 Intel|
|运行时|Node.js、Bun|
|包管理器|npm、pnpm、yarn、Bun|
|产品形态|macOS 原生桌面 App + 菜单栏|
|网络依赖|核心功能完全离线|
|浏览器|不依赖浏览器，不使用 WebView|
|数据存储|本地 JSON 配置文件|
|沙盒|不启用 App Sandbox，不上架 Mac App Store|
|分发|首版自用（开发构建）；稳定后经签名公证通过 GitHub Releases 分发|
|开源协议|MIT|

---

## 2. 技术架构

### 2.1 技术栈

|层级|技术|
|---|---|
|UI 框架|SwiftUI（主界面）+ AppKit（日志视图、菜单栏等性能敏感部分）|
|UI 状态|Observation（`@Observable`）|
|图标|SF Symbols|
|本地化|String Catalog（首版英文，预留简体中文）|
|工程生成|XcodeGen（`project.yml`，不提交 `.xcodeproj`）|
|进程管理核心|Rust（`bunode-core`）|
|异步运行时|Tokio|
|系统信息|libproc / sysinfo（不 shell 调用 `ps` / `lsof`）|
|Swift ↔ Rust|UniFFI（`bunode-ffi`，打包为 XCFramework）|
|Run 守护|`bunode-runner`（Rust 小型二进制）|
|数据存储|JSON 配置文件 + 运行状态文件；日志为滚动文件|

不使用 Tauri / Electron / WebView。选择理由见 [ADR-0001](adr/0001-native-swift-ui-with-rust-core.md)。

### 2.2 架构设计

采用 UI 与进程管理核心分离的架构。

```
Bunode.app
│
├── SwiftUI / AppKit UI
│   ├── Dashboard
│   ├── Projects
│   ├── Processes
│   ├── Logs
│   ├── Settings
│   └── Menu Bar
│
├── bunode-ffi（UniFFI 绑定层）
│
├── bunode-core（Rust，不依赖 UI 与 FFI）
│   ├── Supervisor（Run 生命周期与状态机）
│   ├── Process Discovery
│   ├── Port Scanner
│   ├── Resource Monitor
│   ├── Log Collector（有界环形缓冲区）
│   └── Project Repository
│
└── bunode-runner（每个 Run 一个）
    ├── 创建进程组并启动命令
    └── 生命线管道断开即结束整个进程组
```

MVP 阶段 Supervisor 内嵌在 App 进程中；`bunode-core` 保持与 UI 无关，未来可原样托管到独立的 launchd 常驻进程（见 [ADR-0002](adr/0002-supervisor-embedded-in-app.md)）。

**Swift ↔ Rust 边界**

- **命令**：`start(service)`、`stop(run)` 等调用立即返回，结果通过状态事件回传。
- **状态**：Rust 通过单一回调推送 Run 状态变化，约每 100 ms 合并一次，不逐条推送。
- **日志**：日志保存在 Rust 的有界环形缓冲区；Rust 只通知“该 Run 有 N 条新日志”，Swift 按可见区域范围拉取。

关键要求：

1. 所有进程扫描、端口检测、资源采样和日志读取都必须异步执行。
2. UI 主线程不允许执行阻塞操作。
3. 高频日志不能导致界面持续全量刷新。
4. 进程监控采用增量更新和合理轮询间隔。
5. 每个 Run 具有独立的状态机和生命周期管理。
6. 系统进程发现与 Bunode 创建的 Run 必须分离。
7. 不得因为单个进程异常而影响整个应用。
8. 后端异常不能使 UI 永久停留在 Loading 状态。

---

## 3. 核心功能

### 3.1 Project 与 Service 管理（P0）

**Project** 是在 Bunode 中登记的工作目录；**Service** 是 Project 下一个命名的运行配置（package script 或自定义命令），是用户启停的单位。

**添加 Project**

- 选择本地项目文件夹。
- 手动配置启动命令和工作目录。
- 从已发现的运行中进程创建配置（能够可靠恢复启动参数时才允许）。

**自动识别**

扫描 Project 目录中的：

- `package.json`
- `bun.lock` / `bun.lockb`
- `package-lock.json`
- `pnpm-lock.yaml`
- `yarn.lock`
- `.nvmrc` / `.node-version`（仅用于展示及提示版本不一致，不切换版本）
- `.env` 等环境配置文件（仅展示是否存在）

根据 `scripts` 识别可用 Service，在 GUI 中直接选择运行。

**包管理器识别顺序**

1. `package.json` 的 `packageManager` 字段。
2. 锁文件：`bun.lock(b)` → `pnpm-lock.yaml` → `yarn.lock` → `package-lock.json`。
3. 均无时默认 npm。

同时存在多个锁文件时在 Project 上显示警告，由用户选择；用户的选择覆盖自动识别。

**Monorepo**

添加 workspace 根目录时，整个仓库作为一个 Project；各子包的 scripts 识别为该 Project 下的 Service，每个 Service 记录自己的工作目录（如 `apps/web`），界面显示为 `web › dev`。

**`.env` 策略**

Bunode 不解析、不注入 `.env`，交由 Vite、Next、Bun 等工具自行加载，以保持与终端运行一致。Service 自身配置的环境变量单独管理。

**Project 配置字段**

|字段|说明|
|---|---|
|Name|Project 名称|
|Directory|工作目录|
|Package Manager|Auto / npm / pnpm / yarn / Bun|
|Group|分组|

**Service 配置字段**

|字段|说明|
|---|---|
|Name|Service 名称|
|Working Directory|Project 目录或其子目录|
|Source|package script / 自定义命令|
|Runtime|Auto / Node / Bun|
|Command / Arguments|启动命令与参数（默认可执行文件 + 参数数组）|
|Shell Mode|需要 Shell 特性时显式开启|
|Runtime Path|可覆盖 node / bun 可执行文件路径|
|Environment|环境变量（敏感值默认隐藏）|
|Expected Ports|预期端口|
|Auto Start|自动启动|
|Auto Restart|异常自动重启|

支持编辑、复制、删除 Project 与 Service，以及搜索、排序和分组。

删除配置不能删除项目源代码。删除仍有 Run 在运行的 Project 或 Service 时，需确认“停止 N 个运行中的服务并删除”，先停止相关 Run 再删除，不允许留下无归属的 Run。

### 3.2 Run 生命周期管理（P0）

每个 Service 提供以下操作：

- Start：启动。
- Stop：正常停止。
- Restart：重启（结束当前 Run，开始新的 Run）。
- Force Kill：强制结束。
- Open Folder：打开目录。
- Open in Editor：使用编辑器打开项目。
- Open URL：打开服务监听地址。

**Run 状态**

```
Stopped
   │
   ▼
Starting
   │
   ├──> Running
   │       │
   │       ├──> Stopping ──> Stopped
   │       │
   │       └──> Crashed
   │                  │
   │                  └──> Restarting
   │
   └──> Failed
```

要求：

1. 一个 Project 可以定义多个 Service。
2. 允许多个 Service 同时运行。
3. 每个 Service 同一时间最多一个存活的 Run；防止重复点击导致重复启动。
4. 支持启动、停止、重启过程中的状态反馈。
5. 子进程退出后及时更新状态。
6. 进程退出后正确释放资源。
7. 停止 Run 时向其进程组发送 SIGTERM，超时（默认 5 秒，可配置）后自动升级为 SIGKILL。
8. 必须正确管理进程组，避免 npm、pnpm、Bun 等启动器留下孤儿进程。
9. 防止误杀 PID 被系统复用后的无关进程（同时校验 PID 与启动时间）。
10. 支持崩溃自动重启，并限制重试频率和连续重启次数。
11. Service 运行中修改配置，只对下一次 Run 生效，界面提示“配置已修改，重启后生效”。

**Run 的存活范围**（见 [ADR-0003](adr/0003-runs-never-outlive-bunode.md)）

- 关闭主窗口不结束 Run，菜单栏继续管理。
- 退出 Bunode 时确认后停止所有 Run。
- Bunode 崩溃或被强制结束时，所有 Run 也必须结束：每个 Run 由 `bunode-runner` 启动，生命线管道断开后它结束整个进程组。
- 兜底：Supervisor 持续记录每个 Run 的 PID、进程组与启动时间；下次启动时，身份仍匹配的遗留进程被自动结束。

如需在 GUI 完全退出后继续运行 Service，应通过独立后台 Supervisor 实现（第三阶段），不允许依赖窗口生命周期，也不得为了保留后台进程而让图形窗口始终运行。

### 3.3 自动进程发现（P0）

自动扫描当前用户运行的 Node.js / Bun 相关进程，包括：

- `node`
- `bun`
- `npm`
- `pnpm`
- `yarn`
- 通过这些工具启动的 Vite、Next.js、Nuxt、Astro 等开发服务器。

展示：

|字段|内容|
|---|---|
|Process|进程名称|
|PID|进程 ID|
|Runtime|Node / Bun|
|Project|关联 Project|
|Command|启动命令|
|Directory|工作目录|
|Port|监听端口|
|CPU|CPU 使用率|
|Memory|内存使用量|
|Uptime|运行时间|

进程发现需要考虑父子进程关系，避免将 `npm run dev` 和它启动的 Vite 错误地显示为两个独立服务。

**Run 与 Discovered Process 分开处理：**

- Run：由 Bunode 启动，支持完整生命周期操作。
- Discovered Process：由终端、IDE、Codex 等工具启动，默认提供查看及安全终止功能（仅 SIGTERM；升级为 SIGKILL 需用户确认）。

Discovered Process 位于某个已登记 Project 的目录中时，在该 Project 下显示为“外部运行中”并标出端口，但**永远不被认领为 Service 的 Run**。此时启动对应 Service 会先提示端口或目录冲突。

外部进程只有在能够可靠恢复工作目录、命令、参数及环境的情况下才能提供重启操作。不能将 Kill + 猜测命令重新启动伪装成可靠的 Restart。

### 3.4 实时日志（P0）

支持查看 Run 的标准输出和标准错误。

功能：

- 实时滚动显示日志。
- stdout/stderr 区分。
- 时间戳。
- 关键词搜索。
- 日志级别过滤。
- 自动滚动开关。
- 清空当前显示。
- 复制日志。
- 导出日志文件。
- 支持 ANSI 颜色。

**性能要求**

- 日志采用有界缓冲区（M0：每个 Run 在内存中保留最近约一万行）。
- 大量日志持续输出不能导致 GUI 卡死。
- 使用虚拟化列表渲染，按可见区域拉取。
- 批量更新 UI，禁止每条日志引发一次界面刷新。
- 日志必须支持背压或丢弃策略，并提示丢弃了多少行。
- 磁盘日志文件需要限制大小并支持滚动归档（M1）。
- 外部发现的进程不保证可以追溯历史 stdout/stderr，不得虚构日志内容。

### 3.5 端口管理（P0）

- 自动检测 Run 及 Discovered Process 监听的 TCP 端口。
- 支持一个服务同时监听多个端口。
- 展示端口状态。
- 点击端口打开对应 localhost 地址。
- 启动前检查预期端口冲突。
- 端口冲突时显示占用进程信息。
- 支持用户手动配置预期端口。

不得自动结束不属于 Bunode 的端口占用进程。

### 3.6 资源监控（P1）

展示：

- 实时 CPU 使用率。
- 内存占用。
- 运行时长。
- 进程数量。
- 监听端口。
- 退出码与最近异常。

允许查看单个 Service 的资源使用变化。

默认采用低频采样，在窗口隐藏时主动降低采样频率。

避免为了监控性能而引入明显的系统资源开销。

### 3.7 菜单栏（P0）

提供 macOS Menu Bar 常驻入口。

菜单栏应支持：

- 查看当前运行的 Service 数量。
- 查看全部 Project 与 Service。
- 快速启动/停止 Service。
- 重启 Service。
- 查看异常状态。
- 打开主窗口。
- 退出应用。

菜单栏与主窗口必须共用同一状态来源（Supervisor），避免状态不同步。

**Dock 图标**：主窗口打开时显示 Dock 图标，关闭主窗口后隐藏，仅保留菜单栏；Settings 中可设置为“始终仅菜单栏”。

支持：

- 仅菜单栏模式。
- 菜单栏 + 主窗口模式。
- 启动时最小化到菜单栏。

### 3.8 自动启动与恢复（P1）

支持：

- macOS 登录时启动 Bunode。
- 登录时自动启动指定 Service。
- Bunode 意外退出后清理遗留 Run（见 3.2）。
- 项目异常退出时按策略重启。

区分：

- 用户主动停止。
- 程序异常退出。
- 操作系统重启。
- 主窗口关闭。
- Bunode 退出。

用户主动停止的 Service 不得被自动重启策略立即重新拉起。

### 3.9 运行环境解析（P0）

从 Finder 或登录项启动的 GUI 应用不继承终端的 PATH（fnm、nvm 等）。Bunode 启动时执行一次登录 shell（`$SHELL -l -i -c env`）抓取环境变量并缓存，Settings 中提供“重新加载环境”。每个 Service 可覆盖 node / bun 可执行文件路径。

Bunode 不负责 Node/Bun 版本切换。

---

## 4. UI / UX 设计

### 4.1 设计原则

界面采用现代 macOS 原生设计语言（Liquid Glass、系统控件）。

参考 Raycast、Linear 等产品的视觉语言，但不直接复制。

设计要求：

- 简洁、现代、高信息密度。
- 不使用传统系统进程管理器式的拥挤表格作为唯一主界面。
- 支持深色与浅色主题（跟随系统）。
- 使用系统字体与 SF Symbols。
- 合理使用颜色表示状态。
- 少弹窗、少层级、少复杂配置。
- 支持键盘快捷键。
- 所有常用操作尽量在两次点击内完成。
- 动画轻量，不影响操作响应。
- 首版英文界面；界面用词与 `GLOSSARY.md` 一致（Project / Service / Run）。

### 4.2 主窗口结构

```
┌────────────────────────────────────────────────────┐
│ Bunode                                   ◉  ●  ⚙  │
├───────────────┬────────────────────────────────────┤
│               │                                    │
│ Dashboard     │  Projects                  + Add  │
│ Projects      │                                    │
│ Processes     │  ● api › dev         Bun    :3000  │
│ Logs          │    Running         CPU 2%   86 MB  │
│               │    [Stop] [Restart] [Logs]         │
│ ────────────  │                                    │
│ Groups        │  ● web › dev         Node   :5173  │
│               │    Running         CPU 1%   74 MB  │
│ Work          │    [Stop] [Restart] [Logs]         │
│ Personal      │                                    │
│               │  ○ api › worker      Bun           │
│               │    Stopped                         │
│               │    [Start] [Edit]                  │
│               │                                    │
├───────────────┴────────────────────────────────────┤
│  2 Running     1 Stopped        CPU 3%   MEM 160MB │
└────────────────────────────────────────────────────┘
```

示意内容只用于说明信息层级，不限制最终视觉布局。

### 4.3 页面

**Dashboard**

- 全部 Service 状态概览。
- 正在运行的 Service。
- 异常 Run。
- 常用启动操作。
- 总资源占用。

**Projects**

- Project 与 Service 列表。
- 搜索、分组、排序。
- 添加与编辑 Project / Service。
- 脚本发现。

**Processes**

- Run。
- 自动发现的 Discovered Process。
- PID、端口、CPU、内存。
- 进程树。
- 安全终止操作。

**Logs**

- 按 Service 切换日志。
- 实时日志。
- 搜索与过滤。
- 导出。

**Settings**

- 外观。
- 开机启动。
- 菜单栏与 Dock 图标行为。
- 运行环境（重新加载登录 shell 环境、默认 node/bun 路径）。
- 日志保留策略。
- 进程停止超时。
- 自动重启策略。

---

## 5. 稳定性与性能要求

这是项目的最高优先级。

### 5.1 稳定性

必须重点防范以下问题：

1. 进程持续输出日志导致主窗口冻结。
2. 快速连续点击启停导致状态竞争。
3. 进程启动失败后 UI 一直显示 Starting。
4. 进程已退出，但 UI 仍显示 Running。
5. 停止父进程后遗留 Node/Bun 子进程。
6. 外部进程退出后 PID 复用导致监控对象错乱。
7. 资源监控轮询导致 CPU 占用过高。
8. 端口扫描阻塞 UI。
9. Service 数量增加导致应用响应明显变慢。
10. Bunode 崩溃后遗留 Run 继续运行。

### 5.2 性能目标

以下为开发验收目标，非已验证性能数据。

|指标|目标|
|---|---|
|冷启动|小于 1 秒|
|空闲 CPU|尽量低于 1%|
|空闲内存|优先控制在 50 MB 内|
|常规操作响应|100 ms 内出现反馈|
|进程状态更新|通常 1 秒内|
|同时管理 Service|至少 30 个|
|高负载日志|不冻结 GUI|
|菜单栏唤出|流畅、无明显卡顿|

### 5.3 异常恢复

- 后台操作有明确超时。
- UI 与 Supervisor 断开时显示连接状态。
- 支持重新同步当前进程状态。
- 对意外退出提供诊断日志。
- 不使用无限制的自动重启循环。
- 应用更新或重启不应破坏配置。

---

## 6. 安全要求

- 默认所有操作只在本机执行。
- 不开放远程 HTTP 管理接口。
- 不上传项目目录、日志或环境变量。
- 环境变量中的敏感值默认隐藏。
- 不使用 `sudo` 执行项目命令。
- 不允许普通启动配置隐式获得管理员权限。
- 优先使用可执行文件 + 参数数组启动，避免 Shell 注入。
- 需要 Shell 特性的命令必须显式配置 Shell 模式。
- 终止进程前验证目标 PID、启动时间及所属关系。
- 禁止无确认地批量结束外部进程。

---

## 7. 开发优先级

### M0 — 端到端骨架

- SwiftUI App + Rust 核心 + UniFFI 构建链路打通。
- 添加 Project，识别 `package.json` scripts 为 Service。
- 启动、停止 Service，正确终止进程组（含 `bunode-runner`）。
- 实时日志（内存有界缓冲区）。
- 菜单栏：运行数量与启停。

### M1 — MVP 补全

- Project / Service 编辑、删除。
- Node / Bun 自定义命令启动。
- 重启。
- 自动发现 Node/Bun 进程。
- 端口检测。
- 运行状态持久化与遗留 Run 清理。
- 本地配置持久化。
- 深色/浅色主题。
- 日志落盘与滚动归档。
- 进程管理并发与异常处理测试。

### 第二阶段（增强体验）

- CPU / 内存监控。
- Project 分组。
- 全局快捷键。
- Service 批量启动与停止。
- 环境变量管理。
- 自动重启。
- 登录时启动。
- 进程树。
- 日志搜索与导出。
- 配置导入导出。
- 简体中文界面。

### 第三阶段（扩展能力）

- 独立常驻 Supervisor（launchd）。
- 完全退出 GUI 后继续运行 Service。
- CLI 控制接口。
- MCP Server，供 Codex 等 Agent 查询、启动和停止 Service。
- Service 启动依赖与顺序。
- 工作区配置共享。
- 签名、公证与自动更新。
- 全面的性能测试和异常恢复机制。

---

## 8. 测试与验收

### 8.1 功能测试

必须覆盖：

1. 启动普通 Node.js HTTP 服务。
2. 启动 Bun HTTP 服务。
3. 通过 npm、pnpm 和 Bun 启动 Vite。
4. 同时启动 10 个不同 Service。
5. 连续快速启动和停止同一 Service。
6. 服务启动后立即退出。
7. 服务运行过程中崩溃。
8. 服务持续产生大量日志。
9. 服务生成多级子进程。
10. 服务占用多个端口。
11. 两个服务争用同一端口。
12. 删除正在运行的 Service 的配置。
13. 主窗口关闭、重新打开；Bunode 退出及意外崩溃（验证无遗留进程）。
14. macOS 睡眠、唤醒及用户注销。
15. Node/Bun 可执行文件不存在或版本切换。
16. 项目工作目录被移动或删除。
17. 外部终端启动的 Node/Bun 进程被自动发现。
18. 高频启动、停止操作下的并发安全性。

### 8.2 测试策略

- **Rust 核心**：集成测试真实启动 node / bun 进程，不使用 mock。测试素材放在仓库 `fixtures/`（HTTP 服务、立即崩溃、海量日志、多级子进程、多端口等），对应 8.1。重点验证状态机转移、进程组终止后无遗留进程、PID 复用防护。
- **Swift 层**：只测 ViewModel 与 FFI 适配层，UI 测试保持很少。
- **CI**：GitHub Actions macOS runner 上运行 `cargo test` 并构建 App。

### 8.3 稳定性测试

- 连续运行至少 24 小时。
- 持续高频日志输出 30 分钟。
- 反复启停同一 Service 至少 100 次。
- 同时运行 30 个低负载测试服务。
- 多次关闭和恢复主窗口。
- 模拟 Bunode 异常退出，验证所有 Run 被结束。
- 检查内存泄漏、僵尸进程和遗留进程。
- 检查所有 Run 的最终状态与 UI 是否一致。

### 8.4 验收标准

MVP 必须满足：

- 不依赖终端完成基本 Project / Service 管理。
- Node 和 Bun 都可以稳定启动、停止、重启。
- 支持运行时日志查看。
- 能自动发现外部 Node/Bun 进程。
- 进程异常退出能够正确反映到 UI。
- 不出现不可恢复的 Starting / Stopping 状态。
- 大量日志不能卡死主界面。
- 主窗口关闭后菜单栏仍可管理 Service。
- Bunode 退出或崩溃后不留下任何 Run。
- 不误杀其他应用或无关进程。

---

## 9. 非目标范围

不开发以下功能：

- Docker 容器管理。
- Homebrew 服务管理。
- macOS 全系统 launchd 服务浏览器。
- Kubernetes 管理。
- SSH 远程进程管理。
- 云端项目托管。
- AI 聊天功能。
- 内置代码编辑器。
- 完整终端模拟器。
- Node.js / Bun 安装器及版本管理器。
- 跨平台（Linux / Windows）。
- Mac App Store 上架。

这些能力不是产品核心，不应增加首版复杂度。

---

## 10. 开发原则

1. **稳定性优先于功能数量。**
2. **专注 Node.js / Bun，不演变为通用系统管理器。**
3. **优先复用成熟的 Rust / macOS 系统能力。**
4. **所有影响进程生命周期的操作必须有可测试的状态转移。**
5. **UI 不承担进程生命周期的真实状态管理。**
6. **不允许阻塞 UI 主线程。**
7. **不依赖高频全量轮询实现所有状态同步。**
8. **界面现代、紧凑，避免功能堆砌。**
9. **保持可维护的模块化结构，避免过度设计。**
10. **优先实现完整、稳定的 M0 / MVP，再逐步扩展。**

## 11. 最终交付

- 可运行的 macOS App。
- 完整源代码。
- MIT License。
- README 和构建说明。
- Node/Bun 测试素材（`fixtures/`）。
- 核心进程管理单元与集成测试。
- 自动化构建流程。
- Apple Silicon 和 Intel 安装包。
- GitHub Releases 发布配置。

**最终目标：打造一款比 Quay 更稳定、比 WhatThePort 更美观、更专注 Node/Bun 的 macOS 原生开发服务管理工具。**
