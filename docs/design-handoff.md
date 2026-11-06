# Handoff Spec：Bunode 菜单栏 Popover · Settings 窗口 · 确认对话框

> 来源：设计画布 [Bunode 产品设计](https://claude.ai/artifact/5fhSPrDsLykC49nR2fPhfU)（3 张画板：`MenuBar`、`Settings`、`Dialogs`，版本 `1793949989-deaa`）。
> 术语以 [`GLOSSARY.md`](../GLOSSARY.md) 为准。技术栈：SwiftUI + AppKit（[ADR-0001](adr/0001-native-swift-ui-with-rust-core.md)），状态来自 Rust Supervisor。
> 画布是 HTML 原型，**不是像素规范**：数值给的是意图，落地时一律换成系统控件和语义色（见「设计 Token」）。标 **〔待定〕** 的条目设计没给答案，需要拍板后再做。

---

## 0. 已定的设计决定

设计与 [`requirements.md`](requirements.md) 有 7 处不一致，2026-11-06 按下表定稿，requirements.md 已同步。

| # | 决定 | 落在本文 |
|---|---|---|
| C1 | Popover 是唯一主界面，不做主窗口，始终无 Dock 图标（[ADR-0004](adr/0004-menu-bar-first-no-main-window.md)）。Settings 只有 General / Projects / Advanced | 全文 |
| C2 | 设计里的 `Use login shell` 改为 Service 级 `Run through shell`（即需求的 Shell Mode，用于管道、`&&`、通配符）。登录 shell 环境只在全局抓取（Advanced › Shell environment） | §2.2 Service 视图 |
| C3 | General 的 `Restart Services that crash` 是新 Service 的默认值；Service 级开关覆盖它；改全局不改写已有 Service | §2.3 General |
| C4 | MVP 日志补：隐式自动滚动、丢弃行数提示、`Export…`。级别过滤放到第二阶段 | §2.2 Logs、§7 |
| C5 | 已登记 Project 目录里的 Discovered Process，在该 Project 下显示只读的 External 行 | §2.2 Home |
| C6 | Projects 行可改包管理器，多锁文件时显示警告 | §2.3 Projects |
| C7 | 补 `Failed` 状态：红点 + 行备注 `Failed` | §5.1 |

---

## 1. Overview

Bunode 常驻菜单栏。用户点菜单栏图标打开 Popover，在一张清单里看到所有 Project 和 Service 的状态，一次点击启停，再点一次进入 Service 的设置、日志。全局偏好和 Project 登记放在单独的 Settings 窗口（⌘,）。

三条产品底线（画布 Dialogs 注释），所有交互都要守住：

1. 不留下孤儿 Run：退出 / 崩溃都结束进程组。
2. 不擅自动外部进程：只 SIGTERM，SIGKILL 必须二次确认。
3. 删除配置永远不删源码；按钮和提示要写明这一点。

用户场景：开发者同时开着 3–10 个 dev server，只想快速知道“谁在跑、哪个端口、谁挂了”，偶尔看一眼日志。所以 Popover 追求**一屏、两次点击以内**。

---

## 2. Layout

### 2.1 菜单栏状态项（NSStatusItem）

| 项 | 规格 |
|---|---|
| 图标 | 15×15 pt 六边形描边（1.5 pt）+ 中心 2 pt 半径实心点。做成 template image（描边跟随菜单栏颜色），中心点单独着色 |
| 中心点颜色 | 任一 Service `crashed`/`failed` → `status-error`；否则任一 `running` → `status-running`；否则 `status-idle` |
| 计数 | 图标右侧 5 pt 间距，12 pt semibold，显示 **running** 数量（不含 starting/stopping）。General › `Show running count` 关闭时隐藏；数量为 0 时也隐藏数字（画布显示 0，以本文为准） |
| 按下态 | 背景 `rgba(0,0,0,.12)`，圆角 5 —— 用系统 `NSStatusBarButton` 高亮即可，不要自绘 |
| VoiceOver | `"Bunode, 2 running"`；有崩溃时 `"Bunode, 2 running, 1 crashed"` |

### 2.2 Popover

| 项 | 规格 |
|---|---|
| 容器 | `NSPanel`/`NSPopover`（或 `MenuBarExtra(.window)`），宽 **340 pt 固定**，高度随内容，最大 = 屏幕 `visibleFrame` 高度 − 40 pt（画布 636 pt） |
| 材质 | 系统 popover / Liquid Glass 材质。画布里的 `rgba(246,246,249,.92)` + blur 30 + saturate 1.6 只是模拟，不要硬编码 |
| 圆角 | 14（系统 popover 默认即可） |
| 内边距 | 6 pt 四周 |
| 结构 | 纵向：Header（固定）→ 可滚动内容（`flex:1`）→ 分隔线（1 pt, `separator`, 左右 8 pt 内缩, 上 6 下 4）→ 底部菜单（固定） |
| 视图栈 | `home` → `service` → `logs`；`home` → `addService`；`home` → `processes`。同一时间只显示一个视图，切换时 Popover 宽度不变，高度变化 |

**Home 视图**

```
┌ 6 ─────────────────────────────────────────┐
│  Bunode                  2 running  [▣ 3]  │  header: padding 4/2/6/10
│  api                                   [+] │  Project 小标题: padding 6/2/0/10
│  ● dev                         :3000  [■]  │  Service 行: 高 32
│  ○ worker                             [▶]  │
│  acme                                  [+] │
│  ● web › dev                   :5173  [■]  │
│  ○ docs › dev                  :4321  [▶]  │
│  ● dev                Crashed  :8787  [▶]  │
│ ─────────────────────────────────────────  │
│  Settings…                            ⌘,   │  菜单项: 高 28
│  Quit Bunode                          ⌘Q   │
└────────────────────────────────────────────┘
```

- **Header**：`Bunode`（13 pt semibold，`flex:1`）· `N running`（12 pt，`secondaryLabel`）· 外部进程按钮（芯片图标 14 pt + 数字，高 28，左右 6 pt 内边距，间距 4）。三者间距 8。
- **Project 小标题**：11 pt semibold `secondaryLabel`；右侧 `+` 图标按钮 24×24（图标 12 pt）。
- **Service 行**（容器圆角 8，hover 背景 `fill-hover`）：
  - 主按钮（整行左侧，`flex:1`，高 32，padding 0/4/0/10，间距 8）：状态点 7×7 → 名字（13 pt，单行省略）→ 备注（11 pt，仅 Crashed/Failed 时显示，`status-error-text`）→ 端口（11 pt mono）。
  - 行尾操作按钮 28×26，圆角 7：运行中为 Stop（10 pt 实心圆角方块，`label` 色）；可启动为 Start（10 pt 实心三角，`accent` 色）；过渡态为 12 pt 旋转 spinner，不可点。
  - 端口文字：Run 存活且有实际端口 → 显示实际端口、`accent` 色；否则显示配置端口、`tertiaryLabel` 色；都没有则不显示。
- **External 行**（C5）：Project 下有 Discovered Process 时，排在该 Project 的 Service 行之后。高 28，芯片图标 12 pt + `External · astro dev`（12 pt，`label-secondary`，单行省略）+ 端口（11 pt mono，`label-tertiary`），没有行尾按钮。点击进入 Other processes 并滚动到该进程。只读，永远不当作 Service 的 Run。
- 内容区可滚动；Service 数 ≥ 30（需求 §5.2）时只有内容区滚，Header 和底部菜单固定。

**Service 视图**（点 Service 行进入）

- 顶栏（padding 0/2/8/0，间距 6）：Back 图标按钮 28×28 → 标题块（`project › service`，13 pt semibold 单行省略；副标题 11 pt：6 pt 状态点 + `Running · Run #14 ·` + 可点端口链接）→ `Stop`（次要按钮）或 `Start`（主按钮）。
- 端口冲突提示（如有，见 §4.3）。
- 可滚动内容，三张卡片（卡片：左右外边距 2，下 10，圆角 10，背景 `card-bg`；卡片内行：最小高 38，padding 4/12，间距 10，行间 1 pt `separator`）：
  1. `Command`（输入框 180 宽 mono）· `Folder`（路径 11 pt mono 省略 + `Change…`）· `Port`（副标题 “Checked before each start”，输入框 64 宽右对齐，placeholder `none`）
  2. `Restart if it crashes`（开关，“Pauses after 3 crashes in 60 s”）· `Run through shell`（开关，“For pipes, `&&` and globs”，默认关，C2）· `Environment`（`2 variables` / `None · .env files are not read` + `Edit…`）
  3. `Logs` 整行按钮：右侧 `N lines` + 9 pt chevron
- “已保存，下次 Run 生效”横条（见 §4.4）。
- `Delete Service…` 文字按钮（12 pt，`destructive-text`，外边距 2/10/6）或其内联确认卡片。

**Add Service 视图**（点 Project 小标题的 `+`）

- 顶栏：Back + `New Service` / 副标题 `in acme · runs from ~/code/acme`（路径 mono）。
- 卡片 1（padding 10/12）：`Scripts in package.json`（11 pt）+ 脚本胶囊（高 24，左右 9，圆角 12，11 pt mono，flex-wrap 间距 6）。
- 卡片 2：`Command`（placeholder `bun run dev`）、`Name`（placeholder `dev`），输入框均 180 宽。
- 底部右对齐：`Cancel`（次要）· `Add`（主按钮，命令或名字为空时 disabled，opacity .4）。

**Logs 视图**（Service 视图里点 Logs）

- 顶栏：Back（返回 Service 视图，不是 Home）+ 标题 / 副标题 `Logs · Run #14` 或 `Logs · No Runs yet` + `Restart`（运行中）或 `Start`（主按钮）。
- 搜索框：高 28，圆角 7，背景 `fill-subtle`，左 13 pt 放大镜，placeholder `Search`。外边距 0/2/6。
- 日志区：**AppKit 虚拟化视图**（`NSTableView`/`NSTextView` + 按可见范围拉取，需求 §2.2），画布高 400 pt；背景 `log-bg`（深色，浅色模式下也保持深色）；圆角 9；上下 padding 8；11 pt mono，行高 1.6。每行两列：时间（58 pt 宽，`HH:mm:ss`，`log-time`）+ 正文（自动换行、长单词断行）。列间距 8，行左右 padding 10。
- 底栏（padding 8/2/2，间距 4）：左侧 `N lines` 或 `N lines · M matches`（11 pt）· `Clear` · `Copy`（透明次要按钮）· `Export…`（`NSSavePanel`，导出该 Run 的完整日志文件，默认文件名 `api-dev-run14.log`）。丢弃过行时，左侧计数后加 ` · 1,204 dropped`（同为 `label-secondary`）。

**Other processes 视图**（Header 的芯片按钮，或冲突提示的 `Show Process`）

- 顶栏：Back + `Other processes` / 副标题 `Started outside Bunode · watch only`。
- 一张卡片，每行：命令（13 pt mono 单行省略）+ 副标题 `Terminal · PID 48121 · :4321`（来源 · PID · 端口，无端口则省略）+ `Stop…`。
- 空态：居中 `No other Node or Bun processes.`，上下 padding 30。

### 2.3 Settings 窗口

| 项 | 规格 |
|---|---|
| 窗口 | 标准 `NSWindow`（`Settings` scene），内容 **700×540 pt**，不可调整大小 〔待定：Projects 很多时是否允许纵向拉伸〕 |
| 侧栏 | 宽 168，背景 `sidebar-bg`，padding 14/8/12；交通灯下方留 26。导航项高 32，圆角 8，图标 17 pt + 9 间距 + 13 pt medium 文字。底部 `Source code` 外链（尾部 11 pt ↗） |
| 选中态 | 背景 `accent`、文字白色；落地用 `List(selection:)` + `.sidebar` 样式，系统会给出正确选中色 |
| 主区 | 标题 15 pt bold，padding 20/22/14；内容 padding 0/20/20，可滚动，分组间距 12 |
| 分组卡片 | 圆角 10，背景 `grouped-bg`；行最小高 52，padding 8/10，左右外边距 2，行间 1 pt `separator`；左侧标签（13 pt）+ 帮助（11 pt `secondaryLabel`，单行省略），右侧控件。落地用 `Form { Section {} }.formStyle(.grouped)` |

**General**

| 分组 | 行 | 控件 | 默认 |
|---|---|---|---|
| App | Open at login · “Bunode lives in the menu bar.” | Switch | On 〔待定：首次启动是否默认开〕 |
| App | Show running count · “Next to the menu bar icon.” | Switch | On |
| Runs | Restart Services that crash · “Default for new Services. Up to 3 times a minute.” | Switch | On（C3：只作新 Service 的默认值） |
| Runs | Notify on crash · “One notification per crash.” | Switch | On |

**Projects**

- 一张卡片，每行：名字（13 pt）+ 副标题 `~/code/api · Bun · 2 Services`（路径 mono）+ 包管理器弹出菜单（C6：`Auto (Bun)` / `npm` / `pnpm` / `yarn` / `Bun`，`.controlSize(.small)`）+ `Remove…`。
- 多锁文件时名字后加 `exclamationmark.triangle`（`systemOrange`）图标，tooltip / VoiceOver：`Several lockfiles found. Pick a package manager.`；用户选过后图标消失。改包管理器只影响之后新建的 Service 生成的命令和下一次 Run。
- 卡片下方：左侧说明 `Services are added and set up from the menu bar.`（11 pt）+ 右侧 `Add Project…`（打开 `NSOpenPanel`，只能选文件夹）。
- 空态：卡片内居中 `No Projects yet.`。

**Advanced**

| 分组 | 行 | 右侧 | 行为 |
|---|---|---|---|
| Env | Shell environment · `87 variables from /bin/zsh · today 09:58` | `Reload` 按钮 | 点击后按钮变 `Reloading` + spinner 并 disabled；完成后帮助文字更新为 `just now`，Toast `Environment reloaded. New Runs get it.` |
| Env | Node.js · 路径（mono） | 版本 `22.11.0` + `Change…` | `NSOpenPanel` 选可执行文件 |
| Env | Bun · 路径（mono） | 版本 `1.2.4` + `Change…` | 同上 |
| Stop | Force stop after · “Stop asks nicely first, then force-quits.” | 弹出菜单 `5 s` | 选项 3 / 5 / 10 / 30 s，默认 5（需求 §3.2-7） |
| Stop | Log lines kept · “Per Service. Export saves the full log.” | 弹出菜单 `10,000` | 选项 5,000 / 10,000 / 50,000，默认 10,000 |
| Stop | Diagnostics · “Supervisor and runner logs.” | `Open` | Finder 打开 `~/Library/Logs/Bunode` |

> 画布里弹出菜单是“点一下轮换下一个值”的假控件，落地必须是真正的 `Picker(.menu)`。

### 2.4 确认对话框

用系统 `NSAlert`（或 `.confirmationDialog`/`.alert`），不要自绘。画布尺寸仅作参考：宽 300，圆角 18，App 图标 56，标题 14 bold，正文 12，按钮纵向堆叠高 30、间距 8（这正是 macOS 26 alert 的系统样式）。

| 对话框 | 标题 | 正文 | 主按钮 | 次按钮 |
|---|---|---|---|---|
| 退出（有 Run） | `Quit Bunode?` | `2 Services are running. Bunode stops every Run before it quits, so it never leaves processes behind.` | `Stop 2 Services and Quit`（默认，Return） | `Cancel`（Esc） |
| 删除 Project（有运行中的 Service） | `Remove “api”?` | `Its 2 Services leave Bunode. Files in ~/code/api stay on disk. 1 running Service will be stopped first.` | `Stop 1 and Remove`（destructive） | `Cancel`（默认） |
| 强制结束外部进程 | `Force kill node (PID 51207)?` | `Bunode didn’t start this process. It did not exit 5 s after SIGTERM. Force killing it may lose unsaved work.` + mono 信息框 `next dev -p 3001 · ~/code/landing · started from VS Code` | `Force Kill`（destructive） | `Keep Running`（默认） |

> 画布 Dialogs 板用的是 `Delete “api”?` / `Stop 1 Running Service and Delete`，Settings 板用的是 `Remove` / `Stop N and Remove`。**统一用 Remove**（Project 是“从 Bunode 移除登记”，Delete 容易被读成删文件）；Service 层面沿用 `Delete Service…`（它只存在于 Bunode 里）。

---

## 3. 设计 Token

画布没有接设计系统，颜色是写死的 hex。下表给每个值起名，并映射到 macOS 语义色——**代码里只用右列**，这样深色模式（需求 §4.1 跟随系统）自动成立。

### 3.1 颜色

| Token | 画布值（浅色） | SwiftUI / AppKit | 用途 |
|---|---|---|---|
| `label` | `#1D1D1F` | `.primary` / `labelColor` | 正文、行名 |
| `label-secondary` | `#6C6C70`、`#48484A` | `.secondary` / `secondaryLabelColor` | 帮助文字、计数、副标题 |
| `label-tertiary` | `#8A8A8E` | `tertiaryLabelColor` | 停止态端口、菜单快捷键 |
| `accent` | `#0A63D6` | `Color.accentColor` / `controlAccentColor` | 主按钮、Start 图标、活动端口、开关开、链接 |
| `separator` | `rgba(0,0,0,.07–.08)` | `separatorColor` | 行分隔、Popover 分隔线 |
| `fill-hover` | `rgba(0,0,0,.05)` | `quaternarySystemFill` / 系统 hover | 行 hover |
| `fill-subtle` | `rgba(0,0,0,.05–.06)` | `.quinary` / `controlBackgroundColor` | 搜索框、次要按钮底 |
| `card-bg` | `rgba(255,255,255,.75)` | `.background.secondary` 或 `Material.regular` 上一层 | Popover 卡片 |
| `grouped-bg` | `#F5F5F7` | `Form(.grouped)` 自带 | Settings 分组 |
| `sidebar-bg` | `#EDEDF0` | `.sidebar` 列表样式自带 | Settings 侧栏 |
| `status-running` | `#34C759` | `.green` / `systemGreen` | 运行中 |
| `status-busy` | `#FF9F0A` | `.orange` / `systemOrange` | Starting / Stopping / Restarting（脉冲） |
| `status-error` | `#FF3B30` | `.red` / `systemRed` | Crashed / Failed 点、菜单栏图标中心点 |
| `status-idle` | `#AEAEB2` | `systemGray`（`.gray`） | Stopped |
| `status-error-text` | `#B42318` | `systemRed` 文本变体（深色下要亮） | “Crashed” 备注、`Delete Service…` |
| `destructive` | `#D70015`、`#C4000F`（两处不一致） | `.destructive` 按钮角色 | 删除 / 强杀确认按钮。**统一为系统 destructive，不选 hex** |
| `warning-bg` / `warning-text` | `#FFF4E0` / `#5C3A00` | 自定义 Asset Color（含深色变体） | 端口冲突、SIGTERM 确认内联条 |
| `info-bg` / `info-text` | `rgba(10,99,214,.08)` / `#0A4FA8` | `accentColor.opacity(0.1)` / `accentColor` | “Applies on the next Run” 条 |
| `log-bg` | `#1C1C1E` | 自定义 Asset Color，两种外观都用深色 | 日志区背景 |
| `log-text` / `log-time` / `log-sys` / `log-err` | `#E5E5EA` / `#8E8E93` / `#98989D` / `#FF6961` | 自定义 Asset Color | stdout / 时间戳 / Bunode 系统行 / stderr。ANSI 颜色覆盖 `log-text` |
| `toast-bg` | `rgba(29,29,31,.92)` | 自定义（或 HUD 材质） | Toast |

### 3.2 字体（系统字体 SF Pro / SF Mono）

| Token | 规格 | SwiftUI | 用途 |
|---|---|---|---|
| `font-body` | 13 / regular / 行高 1.35 | `.body`（macOS 13 pt） | 行名、输入标签 |
| `font-body-strong` | 13 / semibold（画布 650） | `.body.weight(.semibold)` | Popover 视图标题 |
| `font-callout` | 12 / regular | `.callout` | 计数、按钮、提示条、Toast |
| `font-caption` | 11 / regular | `.caption` | 帮助文字、副标题、端口 |
| `font-caption-strong` | 11 / semibold | `.caption.weight(.semibold)` | Project 小标题 |
| `font-mono` | 11–12 / SF Mono | `.system(.caption, design: .monospaced)` | 命令、路径、端口、PID、日志 |
| `font-title` | 15 / bold | `.title3.bold()` | Settings 页标题 |
| `font-alert-title` | 14 / bold | `NSAlert` 自带 | 对话框标题 |

### 3.3 间距与圆角

| Token | 值 | 用途 |
|---|---|---|
| `space-2xs` | 2 | 卡片左右外边距、提示条内缩 |
| `space-xs` | 4 | 图标与数字、行纵向 padding |
| `space-sm` | 6 | Popover 内边距、按钮组间距、胶囊间距 |
| `space-md` | 8 | 行内元素间距、视图顶栏下边距 |
| `space-lg` | 10 | 卡片行横向 padding、卡片间距 |
| `space-xl` | 12 | 卡片行 padding、Settings 分组间距 |
| `space-2xl` | 20 | Settings 主区 padding |
| `radius-xs` | 5 | 菜单栏项高亮 |
| `radius-sm` | 6 | 按钮、输入框 |
| `radius-md` | 7–8 | 图标按钮、菜单项、Service 行、提示条、导航项 |
| `radius-lg` | 9–10 | 内联确认卡片、内容卡片、日志区 |
| `radius-pill` | 12 | 脚本胶囊 |
| `radius-popover` | 14 | Popover（系统） |

### 3.4 控件尺寸

| 控件 | 尺寸 | 落地 |
|---|---|---|
| 主 / 次按钮 | 高 26，左右 12 / 10，12 pt | `.controlSize(.small)` + `.borderedProminent` / `.bordered` |
| 图标按钮 | 28×28（行内 28×26；Project `+` 24×24） | `.buttonStyle(.borderless)` + `.help()` |
| 开关 | 34×20，滑块 16 | `Toggle().toggleStyle(.switch).controlSize(.small)` |
| 输入框 | 高 24（Popover）/ 26（Settings），12 pt | `TextField().controlSize(.small)` |
| 状态点 | 7×7（列表）、6×6（标题副行） | 圆形 `Circle()` |

---

## 4. Components

| 组件 | 变体 | 输入 | 说明 |
|---|---|---|---|
| `StatusItemView` | — | `runningCount`, `aggregate: idle/running/error`, `showCount` | §2.1 |
| `ServiceRow` | stopped / busy / running / crashed / failed | `ServiceVM`（见下） | 主按钮打开 Service 视图；行尾按钮按状态切换 |
| `StatusDot` | idle / running / busy（脉冲） / error | `RunStatus`, `size` | 颜色不是唯一信号，见 §9 |
| `ProjectHeader` | — | `name`, `onAdd` | |
| `InlineNotice` | warning / info | `text`, `actions[]` | 端口冲突、SIGTERM 确认、“下次 Run 生效” |
| `InlineConfirm` | neutral / destructive | `text`, `confirmLabel`, `onConfirm`, `onCancel` | Popover 内退出确认、删除 Service 确认。按钮右对齐：Cancel 在左 |
| `SettingsCard` / `SettingsRow` | toggle / picker / button / value+button | `label`, `help`, `control` | 用 `Form(.grouped)` 的 `LabeledContent` |
| `LogView`（AppKit） | — | `runId`, `query` | 虚拟化，按可见范围从 Rust 拉取 |
| `Toast` | — | `message` | 见 §6 |
| `ScriptChip` | default / selected / used(disabled) | `script`, `selected`, `used` | 已有同命令 Service 的脚本 disabled |

`ServiceVM` 字段（UI 只读，全部由 Supervisor 状态推导）：

```
id, projectName, name,           // name 对 monorepo 子包为 "web › dev"
displayName = "\(projectName) › \(name)",
status: stopped|starting|running|stopping|restarting|crashed|failed,
runNumber: Int?, livePort: Int?, configuredPort: Int?,
pid, pgid, startedAt,            // 不显示，用于 PID 复用校验
hasPendingConfig: Bool,          // 运行中改过配置
conflict: DiscoveredProcess?     // 启动前端口检查命中
```

---

## 5. States and Interactions

### 5.1 Service 状态 → 视觉

| 状态 | 点 | 行备注 | 行尾按钮 | Service 视图顶栏 | Logs 顶栏 |
|---|---|---|---|---|---|
| Stopped | `status-idle` | — | Start | `Stopped` + `Start` | `Start` |
| Starting | `status-busy` 脉冲 | — | spinner（不可点） | `Starting…`，无按钮 | 无按钮 |
| Running | `status-running` | — | Stop | `Running · Run #N ·` `:port` 链接 + `Stop` | `Restart` |
| Stopping / Restarting | `status-busy` 脉冲 | — | spinner | `Stopping…` / `Restarting…` | 无按钮 |
| Crashed | `status-error` | `Crashed` | Start | `Crashed` + `Start` | `Start` |
| Failed | `status-error` | `Failed` | Start | `Failed` + `Start` | `Start` |

规则：

- **防重复点击**：Start 后按钮立即变 spinner，直到 Supervisor 推回 `running`/`failed`（需求 §3.2-3）。UI 不自己计时。
- 100 ms 内必须有可见反馈（需求 §5.2）：本地乐观地把点切到 busy，真实状态以 Supervisor 事件为准。
- 后端超时（需求 §2.2-8）：Starting/Stopping 超过 Force stop 时长 + 5 s 未收到事件 → 行备注显示 `Not responding`，按钮恢复，可再点 Stop 触发 Force Kill。

### 5.2 元素交互

| 元素 | 状态 | 行为 |
|---|---|---|
| 菜单栏图标 | 点击 | 开 / 关 Popover。再次打开时总是回到 `home` 视图 |
| Service 行 | Hover | 背景 `fill-hover`，100 ms 过渡 |
| Service 行 | 点击 | 进入 Service 视图 |
| Start / Stop 图标 | 点击 | 直接启停，不确认；Stop 在 `running` 才出现 |
| Project `+` | 点击 | 进入 Add Service，默认选中第一个“未被使用”的脚本，命令按包管理器生成：Bun `bun run x`、pnpm `pnpm x`、npm `npm run x`、yarn `yarn x` |
| 脚本胶囊 | 点击 | 选中，同时填好 Command 与 Name；之后手改 Command 会取消胶囊选中 |
| `Add` | 可用 | 命令和名字都非空。成功后回 Home，Toast `Added acme › storybook` |
| `Add` | Disabled | opacity .4，无 hover |
| 端口链接 | 点击 | 浏览器打开 `http://localhost:<livePort>`（需求 §3.5） |
| `Folder › Change…` | 点击 | `NSOpenPanel`，起始目录为 Project 根，**只允许选 Project 目录或其子目录**（需求 §3.1），选到外面时提示并拒绝 |
| `Port` 输入 | 输入 | 只接受数字，最多 5 位，1–65535；空 = 无预期端口 |
| `Environment › Edit…` | 点击 | 〔待定〕打开 key = value 编辑表（Popover 内新视图或 sheet）。值默认遮罩（需求 §6） |
| `Logs` 行 | 点击 | 进入 Logs 视图，清空搜索词 |
| `Delete Service…` | 点击 | 原位替换为 `InlineConfirm`（见 §5.5） |
| 芯片按钮 | 点击 | 进入 Other processes |
| `Stop…`（外部进程） | 点击 | 行下展开 warning 条 `Send SIGTERM to PID 51207? Bunode didn’t start it and can’t bring it back.` + `Cancel` / `Send SIGTERM`（`label` 色深底按钮） |
| `Send SIGTERM` | 确认 | 行变 opacity .45 + `Exiting…`；进程退出后从列表移除。**5 s 后仍存活 → 弹 §2.4 Force kill 对话框**（画布 Popover 未画这一步，以 Dialogs 板为准） |
| `Settings…` | 点击 / ⌘, | 打开或前置 Settings 窗口，关闭 Popover |
| `Quit Bunode` | 点击 / ⌘Q | 无 Run → 直接退出；有 Run → 确认（见 §5.6） |

### 5.3 端口冲突（启动前检查）

触发：Start 一个配置了 `Port` 的 Service，而该端口被 Discovered Process 占用。

- 不启动。在该行下方（Home）或顶栏下方（Service 视图）插入 warning 条：
  `Port 4321 is used by “astro dev” from Terminal. Bunode won’t stop it.`
- 按钮：`Use :4322`（主）· `Show Process`（白底次要，跳 Other processes）· `Cancel`（透明）。
- `Use :N` 的 N = 从配置端口 +1 起第一个空闲端口（画布只是 +1）。
- 端口传递：只为本次 Run 设置环境变量 `PORT=4322`，不改写命令参数（需求 §3.5）。条内正文下方加一行 11 pt：`Sets PORT=4322 for this Run. Tools that ignore PORT need their own flag.` Service 的 Port 配置不变，下一次 Run 仍先检查原端口。
- 同一时间只显示一个冲突条；点其他 Service 的 Start 会替换它。
- 绝不结束占用进程（需求 §3.5）。

### 5.4 运行中修改配置

- 所有字段**失焦或回车即自动保存**，无 Save 按钮。
- Service 存活时任一字段改动 → 卡片下方出现 info 条 `Saved. Applies on the next Run.` + `Restart now`（链接样式 12 pt semibold）。
- 点 `Restart now` 或 Run 结束后条消失（需求 §3.2-11）。

### 5.5 删除 Service（Popover 内）

- 确认卡片文案：
  - 运行中：`It is running and will be stopped first. Only the Service is removed; files in ~/code/api stay.`，按钮 `Stop and Delete`
  - 未运行：`Only the Service is removed; files in ~/code/api stay.`，按钮 `Delete`
- 确认后：先 Stop（等 Supervisor 确认 stopped），再删配置 → 回 Home → Toast `Deleted api › worker. Files in ~/code/api are untouched.`
- 离开视图（Back）时确认卡片自动收起。

### 5.6 退出

- Popover 内 `Quit Bunode`：底部菜单上方插入 `InlineConfirm`：`2 Services are running. Quitting stops every Run.` + `Cancel` / `Stop 2 Services and Quit`。确认后换成 spinner `Stopping all Runs…`，全部停止后退出。
- 其他入口的 ⌘Q（Settings 窗口在前时、Dock 菜单、系统注销）：用 §2.4 的 `NSAlert`。
- 两种确认都保留：Popover 打开时用内联，其余入口用 alert，按钮文案统一为 `Stop N Services and Quit`。
- 系统关机 / 注销（`applicationShouldTerminate` 来自 `NSWorkspace` power-off）：不弹确认，直接停止所有 Run（需求 §3.8 区分 OS 重启）。

### 5.7 Settings 交互

| 元素 | 行为 |
|---|---|
| 侧栏项 | 切换页面，关闭任何打开的 sheet |
| 开关 | 立即生效并持久化；150 ms 过渡 |
| `Remove…`（Project） | 弹 §2.4 alert（画布是窗口内 sheet：340 宽、距顶 120、背景遮罩 `rgba(0,0,0,.16)` —— 用系统 sheet 即可）。无 Run 时按钮 `Remove`，有 Run 时 `Stop N and Remove` |
| Remove 确认后 | 先停 Run，再删登记 → Toast `Removed api. Files untouched.` |
| `Add Project…` | `NSOpenPanel` 选文件夹 → 识别包管理器 → 加入列表 → Toast `Added ~/code/invoice-bot (Bun). Add its Services from the menu bar.`；已登记 → Toast `invoice-bot is already added` |
| `Source code` | 浏览器打开 `https://github.com/imal1/bunode` |

---

## 6. 内容规范

### 6.1 截断

| 文本 | 规则 |
|---|---|
| Service 名 / 视图标题 | 单行，尾部省略。全名放进 tooltip（`.help`）与 VoiceOver |
| 路径（Folder、Projects 行、Node/Bun 路径） | 单行，**中间省略**（`truncationMode(.middle)`），保留末段目录名 —— 画布是尾部省略，换成中间更有用 |
| 外部进程命令 | 单行尾部省略，tooltip 显示完整命令 |
| 帮助文字 | 单行尾部省略（画布如此）。中文本地化后可能过长，见 §7 |
| 日志行 | 自动换行，长 token 任意断行 |
| 对话框 meta 框 | 自动换行，任意断行 |

### 6.2 字数与校验

| 字段 | 限制 |
|---|---|
| Service Name | 1–40 字符，Project 内唯一（不唯一时 `Add` disabled，名字框下 11 pt 红字 `Name already used in acme` 〔设计未画〕） |
| Command | 非空，不限长度，单行输入框横向滚动 |
| Port | 1–65535 |

### 6.3 空态

| 位置 | 文案 |
|---|---|
| Home 无 Project 〔设计未画〕 | 居中：`No Projects yet.` + 主按钮 `Add Project…`（打开 Settings › Projects 并触发选择） |
| Project 下无 Service 〔设计未画〕 | 小标题下一行 11 pt：`No Services. Click + to add one.` |
| Logs 无输出 | `No output yet.` |
| Logs 搜索无结果 | `No lines match.` |
| Other processes 为空 | `No other Node or Bun processes.` |
| Settings Projects 为空 | `No Projects yet.` |
| Add Service 无 package.json 〔设计未画〕 | 隐藏脚本卡片，Command 直接聚焦 |

### 6.4 Toast

- 位置：所在窗口底部居中，距底 16–18；padding 7–8/14；圆角 9–10；最大宽 380（Popover）/ 460（Settings）。
- 2.4 s 后自动消失；新 Toast 替换旧的。`role=status`（VoiceOver 播报，见 §9）。
- 文案见 §5。原型里“Opens a file picker”之类 Toast 只是占位，真实行为替代它们。

### 6.5 数字格式

- 端口 `:3000`（mono，冒号前缀）。Run 号 `Run #14`。日志计数 `250 lines`、单复数正确（`1 line`）。
- `10,000` 用系统 `NumberFormatter`（随 locale）。时间戳 `HH:mm:ss` 24 小时。

---

## 7. 边界情况

- **30+ Service**：Home 内容区滚动；Popover 不超过屏幕高。打开时滚动到顶部 〔待定：是否先显示 Crashed〕。
- **长名字 / monorepo**：`acme › storybook-docs-preview › dev` 这类显示名在 340 pt 下必然截断，tooltip 必须有。
- **国际化**：首版英文，预留简体中文（需求 §2.1）。中文帮助文字一般更短；德语等更长，帮助文字截断可接受，但按钮文案（`Stop 2 Services and Quit`）不可截断——允许按钮变宽，alert 按钮纵向堆叠本来就容纳得下。所有带数量的文案用 String Catalog 复数规则，不要字符串拼接。
- **高频日志**：日志区只追加，不全量刷新；用户向上滚动离开底部时暂停自动滚动，回到底部恢复（隐式自动滚动开关，C4）。丢弃行时在底栏计数后显示 ` · 1,204 dropped`。
- **崩溃循环**：60 s 内崩 3 次后暂停自动重启，日志写系统行 `Crashed 3 times in 60 s. Auto restart paused.`，行显示 Crashed，按钮 Start。
- **Project 目录被移动 / 删除**（需求 §8.1-16）〔设计未画〕：Project 小标题旁显示警告图标，Service Start 时 warning 条 `~/code/api is missing.` + `Locate…`。
- **node / bun 不存在**（需求 §8.1-15）：Run 进入 Failed，日志系统行写明找不到哪个可执行文件，Advanced 的对应行帮助文字变红 `Not found`。
- **外部进程在冲突条显示期间退出**：冲突条自动消失。
- **Supervisor 断连**（需求 §5.3）〔设计未画〕：Popover 顶部 warning 条 `Lost connection to the Supervisor.` + `Resync`；所有启停按钮 disabled。
- **Popover 打开时状态变化**：行原地更新，不重排顺序（顺序 = Project 登记顺序 + Service 创建顺序）。

---

## 8. Animation / Motion

| 元素 | 触发 | 动画 | 时长 | 缓动 |
|---|---|---|---|---|
| 状态点 | busy 状态 | opacity 1 → .25 → 1 循环 | 1 s / 周期 | ease-in-out |
| Spinner | busy、Reloading、Stopping all | 旋转 360° 循环 | 0.9 s | linear（用系统 `ProgressView().controlSize(.small)` 即可） |
| 开关 | 切换 | 底色 + 滑块位移 14 pt | 150 ms | ease |
| 行 hover | 鼠标进入 | 背景色 | 100 ms | ease |
| 视图切换 | 进入子视图 / Back | 横向推入（进入从右、Back 从左）；Popover 高度变化跟随系统 | 150 ms | ease-out |
| Toast | 出现 / 消失 | 淡入 / 淡出 | 150 ms / 200 ms | ease-out |
| 外部进程行 | SIGTERM 已发 | opacity → .45 | 即时 | — |

开启“减弱动态效果”（Reduce Motion）时：状态点不脉冲（改为静态 busy 色 + spinner 已足够表达），视图切换无位移。

---

## 9. Accessibility

画布颜色对比实测（WCAG 2.1 AA，正文需 4.5:1，非文字 UI 需 3:1）：

| 组合 | 比值 | 结论 |
|---|---|---|
| `#6C6C70` 帮助文字 on 卡片 | 4.8 | 通过 |
| `#8A8A8E` 停止态端口 / 快捷键 on popover | 3.2 | **不通过**（11–12 pt）→ 改用 `secondaryLabel` |
| `#0A63D6` 活动端口 on popover | 5.2 | 通过 |
| 白字 on `#0A63D6` 主按钮 | 5.6 | 通过 |
| 状态点 `#34C759` / `#FF9F0A` / `#AEAEB2` on popover | 2.1 / 1.9 / 2.1 | **不通过** 3:1 → 状态不能只靠点的颜色，见下 |
| 日志各色 on `#1C1C1E` | 5.2–6.0 | 通过 |

要求：

- **状态不只靠颜色**：每行的 VoiceOver label 包含状态（`"api › dev, Running, port 3000"`）；视觉上 Crashed/Failed 有文字备注，busy 有 spinner；Stopped/Running 由行尾 Start/Stop 图标区分。Stopped 点用空心圆（1.5 pt 描边，ASCII 稿里就是 `○`），增加形状差异。
- **Service 行**：画布是“一个大按钮 + 一个图标按钮”。落地为一个 accessibility 元素：label 如上，默认动作 = 打开设置，`accessibilityAction(named: "Start"/"Stop")` 暴露启停，避免读两遍。
- **图标按钮都要 label + tooltip**：`Stop api › dev`、`Start api › dev`、`Add Service to acme`、`Back` / `Back to api › dev`、`Other Node and Bun processes, 3`、`Open :3000 in browser`。
- **开关**：label = 行标题，value = on/off；帮助文字作为 `accessibilityHint`。
- **日志区**：`accessibilityRole = .textArea`/log，label `Logs of api › dev`；新行**不**自动播报（会刷屏）。
- **Toast / 内联提示**：出现时用 `NSAccessibility.post(.announcementRequested)` 播报一次；冲突条、确认卡片出现时焦点移到其主按钮。
- **焦点顺序（Popover Home）**：Header 芯片按钮 → 每个 Project：`+` → 每个 Service：行 → 行尾按钮 → 底部 Settings… → Quit。打开 Popover 时焦点在第一个 Service 行。
- **键盘**（设计未画，按需求 §4.1 补齐）：

| 键 | 位置 | 行为 |
|---|---|---|
| ↑ / ↓ | Home | 在 Service 行间移动 |
| Return | Home 行 | 打开 Service 视图 |
| Space 或 ⌘R | Home 行 | 启动 / 停止（⌘R 运行中为 Restart） |
| Esc | 子视图 | 返回上一级；Home 时关闭 Popover；有内联确认时先关确认 |
| ⌘F | Logs | 聚焦搜索框 |
| ⌘K | Logs | Clear |
| ⌘, / ⌘Q | 任意 | Settings / Quit |

- 对话框：Return = 默认按钮，Esc = Cancel/Keep Running。**破坏性操作（Remove、Force Kill）的默认按钮是取消**，退出对话框的默认按钮是 `Stop N Services and Quit`（画布注释明确如此）。
- 焦点环：系统默认（画布模拟为 3 pt `accent` 35% 外描边）。

---

## 10. 文案清单（给 String Catalog）

```
Popover
  "Bunode" · "%d running" · "Settings…" · "Quit Bunode"
  "Crashed" · "Failed" · "Starting…" · "Running" · "Stopping…" · "Restarting…" · "Stopped"
  "Run #%d" · "No Runs yet"
  "Port %d is used by “%@” from %@. Bunode won’t stop it." · "Use :%d" · "Show Process" · "Cancel"
  "Command" · "Folder" · "Change…" · "Port" · "Checked before each start" · "none"
  "Restart if it crashes" · "Pauses after 3 crashes in 60 s"
  "Run through shell" · "For pipes, && and globs"
  "External · %@" · "Not responding" · "Sets PORT=%d for this Run. Tools that ignore PORT need their own flag."
  "Environment" · "%d variables" · "None · .env files are not read" · "Edit…"
  "Logs" · "%d lines" · "Saved. Applies on the next Run." · "Restart now"
  "Delete Service…" · "It is running and will be stopped first. " · "Only the Service is removed; files in %@ stay."
  "Stop and Delete" · "Delete" · "Deleted %@. Files in %@ are untouched."
  "New Service" · "in %@ · runs from %@" · "Scripts in package.json" · "Name" · "Add" · "Added %@"
  "Logs · %@" · "Search" · "No output yet." · "No lines match." · "%d lines · %d matches" · "Clear" · "Copy" · "Export…" · "%d dropped" · "Restart" · "Start" · "Stop"
  "Other processes" · "Started outside Bunode · watch only" · "Stop…" · "Exiting…"
  "Send SIGTERM to PID %d? Bunode didn’t start it and can’t bring it back." · "Send SIGTERM" · "SIGTERM sent to PID %d"
  "No other Node or Bun processes."
  "%d Services are running. Quitting stops every Run." · "Nothing is running." · "Stop %d Services and Quit" · "Quit" · "Stopping all Runs…"
Settings
  "General" · "Projects" · "Advanced" · "Source code"
  "Open at login" · "Bunode lives in the menu bar."
  "Show running count" · "Next to the menu bar icon."
  "Restart Services that crash" · "Default for new Services. Up to 3 times a minute."
  "Notify on crash" · "One notification per crash."
  "Remove…" · "Auto (%@)" · "Several lockfiles found. Pick a package manager." · "No Projects yet." · "Services are added and set up from the menu bar." · "Add Project…"
  "Remove “%@”?" · "Its %d Services leave Bunode. Files in %@ stay on disk." · " %d running Services will be stopped first."
  "Stop %d and Remove" · "Remove" · "Removed %@. Files untouched."
  "Shell environment" · "%d variables from %@ · %@" · "Reload" · "Reloading" · "Environment reloaded. New Runs get it."
  "Node.js" · "Bun" · "Force stop after" · "Stop asks nicely first, then force-quits."
  "Log lines kept" · "Per Service. Export saves the full log." · "Diagnostics" · "Supervisor and runner logs." · "Open"
Dialogs
  "Quit Bunode?" · "%d Services are running. Bunode stops every Run before it quits, so it never leaves processes behind."
  "Stop %d Services and Quit"
  "Force kill %@ (PID %d)?" · "Bunode didn’t start this process. It did not exit %d s after SIGTERM. Force killing it may lose unsaved work."
  "Force Kill" · "Keep Running"
```

所有 `%d` 文案走复数规则（`1 Service` / `2 Services`）。

---

## 11. 待定事项汇总

C1–C7 及端口传递、退出确认、键盘、动效、未画状态均已定（§0 及正文）。仍需决定：

1. Settings 窗口是否允许纵向拉伸（§2.3）。
2. `Open at login` 首次启动是否默认开（§2.3）。
3. Environment 编辑器形态：Popover 内新视图，还是 sheet（§5.2）。
4. Service 很多时，打开 Popover 是否先滚到 Crashed / Failed 的行（§7）。
