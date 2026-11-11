# Handoff Spec：Bunode 菜单栏 Popover · Settings 窗口 · 确认对话框

> 来源：设计画布 [Bunode 产品设计](https://claude.ai/artifact/5fhSPrDsLykC49nR2fPhfU)（画板：`MenuBar`、`Settings`、`Dialogs`、`Interactions`，版本 `1794194603-baf2`，2026-11-09）。
> 术语以 [`GLOSSARY.md`](../GLOSSARY.md) 为准。技术栈：SwiftUI + AppKit（[ADR-0001](adr/0001-native-swift-ui-with-rust-core.md)），状态来自 Rust Supervisor。
> 画布是 HTML 原型，**不是像素规范**：数值给的是意图，落地时一律换成系统控件和语义色（见「设计 Token」）。设计没画、由本文补齐的部分标注〔设计未画〕。
> **本文描述目标界面，当前代码还是上一版。** 差异见 §0.2。代码注释按章节号引用本文，调整结构时不要改动 §2–§9 的编号。

---

## 0. 已定的设计决定

设计与 [`requirements.md`](requirements.md) 的不一致，2026-11-06 按下表定稿。第三列是 2026-11-09 新设计带来的变化。

| # | 决定 | 本版状态 | 落在本文 |
|---|---|---|---|
| C1 | Popover 是唯一主界面，不做主窗口，始终无 Dock 图标（[ADR-0004](adr/0004-menu-bar-first-no-main-window.md)）。Settings 只有 General / Projects / Advanced | 不变 | 全文 |
| C2 | 设计里的 `Use login shell` 改为 Service 级 `Run through shell`（即需求的 Shell Mode，用于管道、`&&`、通配符）。登录 shell 环境只在全局抓取（Advanced › Shell environment） | 不变。新画布又写成 `Use login shell`，以本条为准 | §2.2 Service 视图 |
| C3 | General 的 `Restart Services that crash` 是新 Service 的默认值；Service 级开关覆盖它；改全局不改写已有 Service | 不变 | §2.3 General |
| C4 | MVP 日志补：隐式自动滚动、丢弃行数提示、`Export…`。级别过滤放到第二阶段 | **已取消**（2026-11-11）。日志改在控制台里看，没有 Logs 视图（[ADR-0008](adr/0008-logs-open-in-console.md)） | §2.2 Logs、§7 |
| C5 | 已登记 Project 目录里的 Discovered Process，在该 Project 下显示只读的 External 行 | **已取消**（2026-11-09）。没有 Project 标题行后无处安放，统一进 Other processes | §2.2 Home |
| C6 | Projects 行可改包管理器，多锁文件时显示警告 | **待拍板**（§11） | §2.3 Projects |
| C7 | 补 `Failed` 状态，行备注 `Failed` | 保留状态。**“置灰”改为红色色块**，与 Crashed 相同 | §5.1 |

### 0.1 本版新增的决定

| # | 决定 | 原因 |
|---|---|---|
| D1 | 一条命令一个条目，去掉 Project 标题行，Project 降到第二行 | 用户找的是“哪条命令”，Project 只是归属。同 Project 的条目仍相邻 |
| D2 | 两行条目，参考 Llama app 的 Popover | 严格一行的版本试过两轮，Project 放哪都不好看 |
| D3 | 状态放在左侧 32 pt 色块上，第二行用文字再说一次 | 状态是扫一眼最想知道的信息，占最大面积。文字保证不靠颜色也能读 |
| D4 | Start / Stop 只在悬停或键盘聚焦时出现 | 静止时每行右侧只有浅色箭头，清单更轻。键盘和读屏路径见 §9 |
| D5 | Settings 严格只到 Project 层级，Service 设置只能从 Popover 进入 | 全局设置保持极简 |
| D6 | 新建 Service 的入口是 Popover 顶栏的 `Add Service` 文字链 | Project 标题行和它的 `+` 没了 |
| D7 | Home 可在 Settings 里切成按 Project 分组：每组一个只写名字的小标题，组内行标题去掉 Project 名。默认仍是平铺（2026-11-10 定） | 同一个 Project 的命令多时，分组更好找；平铺适合命令少、跨 Project 扫一眼 |
| D8 | agent 启动的开发服务（Agent Server，需求 §3.10）显示成普通行，跟所属 Project 放在一起，可以 Stop，不能 Start。没有徽标，只在 tooltip 写 `Started by Claude` | 用户只关心 agent 里的服务在不在跑、能不能停。这个概念不需要出现在界面上 |
| D9 | 界面英文和简体中文，Settings › General › Language 切换，重启后生效（[ADR-0006](adr/0006-language-switch-needs-restart.md)） | |
| D10 | Service 的命令可以是任意可执行文件（需求 §1.1）。界面不区分 Node/Bun 命令和其他命令 | napcat 这类命令也要一键启停、看日志 |

### 0.2 相对现有代码要改什么

本表没提到的行为，沿用现有实现。

| 位置 | 现在 | 改成 | 文件 |
|---|---|---|---|
| Home 结构 | 按 Project 分组，有小标题和 `+` | 平铺的 Service 行，顺序仍是 Project 登记顺序 + Service 创建顺序 | `HomeView.swift` `ProjectSection` |
| 行主文字 | `displayName` | `command`，包管理器前缀变灰。`displayName` 只留给 tooltip、VoiceOver、日志标题 | `ServiceRow` |
| 行高和左侧 | 32 pt，7 pt 状态点 | 46 pt，32 pt 色块，两行文字 | `ServiceRow` |
| 行尾 | 常驻 Start / Stop，busy 时 spinner | 静止时箭头，悬停或键盘聚焦时换成 Start / Stop。busy 时 spinner 移到色块，行尾留箭头 | `RowAction` |
| Crashed / Failed | 置灰 | 红色色块，第二行状态词红色 | `RunStatus+UI.swift` |
| Home 顶栏 | Bunode + running + 进程数芯片 | 两行标题，右侧文字链 `Add Service` | `HomeView.header` |
| Other processes 入口 | 顶栏芯片 | 清单下方固定的两行条目 | `HomeView` |
| External 行 | 每个 Project 下只读显示 | Home 不显示（C5） | `ExternalRow` |
| Add Service 路由 | `.addService(projectID:)` 从 Project 进入 | 从顶栏进入，默认第一个 Project，页面内用胶囊切换。`Name` 字段去掉 | `AddServiceView` |
| 底部 | 两行菜单，带 ⌘ 提示 | 版本信息 + `Settings` `Quit` 两个小按钮。⌘, 和 ⌘Q 继续生效 | `PopoverView.MenuRow` |
| 打开时焦点 | 第一行有焦点框 | 打开时不显示焦点框，见 §9 | `HomeView.focusFirstRow` |
| 滚动上限 | 屏幕可见高度 − 40 − 120 | 120 改为约 156，见 §2.2 | `PopoverScroll.maxHeight` |
| 分组模式 | 无 | General › `List` 为 `By Project` 时插入 Project 小标题，组内行标题去掉 Project 名（§2.2） | `HomeView.swift`、`RunStatus+UI.swift` `title(projectName:)` |
| Agent Server 行 | 无 | 新增，见 §2.2。从 Other processes 里去掉这些进程 | `HomeView.swift`、`OtherProcessesView.swift`、`bunode-core` 新模块（识别、读 `.claude/launch.json`、停止） |
| Settings › General | 4 个开关 | 加 `List`、`Language` 两行（§2.3） | `SettingsView.swift` |
| Settings › Projects | 无 | 卡片下方加 agent 开关（§2.3） | `ProjectsPane.swift` |
| 文案 | 英文写死在视图里，部分错误文字来自 Rust | 全部进 String Catalog，加简体中文。Rust 只返回类型化错误（枚举 + 参数），Swift 本地化 | `Localizable.xcstrings`、`bunode-core` `error.rs` |

---

## 1. Overview

Bunode 常驻菜单栏。用户点菜单栏图标打开 Popover，一眼看到每条命令是否在跑，鼠标移到行上一键启停，点击行进入这个 Service 的设置和日志。全局偏好和 Project 登记放在单独的 Settings 窗口（⌘,）。

三条产品底线（画布 Dialogs 注释），所有交互都要守住：

1. 不留下孤儿 Run：退出 / 崩溃都结束进程组。
2. 不擅自动外部进程：只 SIGTERM，SIGKILL 必须二次确认。
3. 删除配置永远不删源码；按钮和提示要写明这一点。

此外：运行中修改配置，下一次 Run 才生效；Bunode 不读取 `.env`。

用户场景：开发者同时开着 3–10 个 dev server，只想快速知道“谁在跑、哪个端口、谁挂了”，偶尔看一眼日志。所以 Popover 追求**一屏、两次点击以内**。

---

## 2. Layout

### 2.1 菜单栏状态项（NSStatusItem）

| 项 | 规格 |
|---|---|
| 图标 | 15×15 pt 六边形描边（1.5 pt）+ 中心 2 pt 半径实心点。做成 template image（描边跟随菜单栏颜色），中心点单独着色 |
| 中心点颜色 | 任一 Service `crashed`/`failed` → `status-error`；否则任一 `running` → `status-running`；否则 `status-idle`。Popover 顶栏的 7 pt 点用同一规则 |
| 计数 | 图标右侧 5 pt 间距，12 pt semibold，显示 **running** 数量（不含 starting/stopping，也不含短时 Service（§5.1）和 Agent Server（§2.2））。General › `Show running count` 关闭时隐藏；数量为 0 时也隐藏数字（画布显示 0，以本文为准） |
| 按下态 | 用系统 `NSStatusBarButton` 高亮，不要自绘（画布模拟值：高 22，圆角 5，黑 12%） |
| VoiceOver | `"Bunode, 2 running"`；有崩溃时 `"Bunode, 2 running, 1 crashed"` |

### 2.2 Popover

| 项 | 规格 |
|---|---|
| 容器 | `NSPopover`，宽 **340 pt 固定**，高度随内容。画布的 636 是画板限制，不是规格 |
| 背景 | 系统 NSPopover 默认材质，内容层不铺底色 |
| 圆角、阴影 | 系统 NSPopover，不要自绘（画布模拟 14 圆角和投影） |
| 箭头 | 不要。顶边贴菜单栏下沿（2026-11-09 定） |
| 内边距 | 6 pt 四周，行宽 = 340 − 12 = 328 |
| 结构 | 纵向：顶栏（固定）→ Service 列表（滚动）→ Other processes（固定）→ 底栏（固定） |
| 视图栈 | `home` → `service` → `logs` / `environment`；`home` → `addService`；`home` → `processes`。同一时间只显示一个视图，切换时宽度不变，高度变化 |

**Home 视图**

```
┌ 6 ─────────────────────────────────────────┐
│  Bunode ●                      Add Service │  顶栏: padding 4/8/8/10
│  2 running · 6 Services                    │
│ ─────────────────────────────────────────  │
│ [▣] api dev  :3000                      ›  │  Service 行: 高 46
│     ~/code/api · Running                   │
│ [▣] bun run worker                      ›  │
│     ~/code/api                             │
│ [◌] pnpm --filter web dev               ›  │
│     ~/code/acme · Starting…                │
│ [!] mcp-notes dev                       ›  │
│     ~/code/mcp-notes · Crashed             │
│ ─────────────────────────────────────────  │
│ [⌕] Other processes                     ›  │  固定，高 46
│     3 started outside Bunode               │
│ ─────────────────────────────────────────  │
│  0.1.0 · Bun 1.2.4 · Node 22.11  [Settings] [Quit] │  底栏: 高 28
└────────────────────────────────────────────┘
```

| 区块 | 尺寸 | 内容 | 滚动 |
|---|---|---|---|
| 顶栏 | padding 上 4 右 8 下 8 左 10，约 47 高 | 第一行 `Bunode`（14 semibold）+ 7 pt 状态点，间距 6。第二行 `{running} running · {total} Services`（12，`label-secondary`，`running` 与菜单栏计数相同）。右上 `Add Service` 文字链 12.5，`accent`，悬停下划线 | 固定 |
| 分隔线 | 1 pt，左右缩进 8，下边距 4 | `separator` | 固定 |
| Service 列表 | 每行 46，行间距 0 | 见下 | **只有这一区滚动** |
| 分隔线 | 上下边距 4，左右缩进 8 | | 固定 |
| Other processes | 46 | 色块（放大镜）+ 标题 + 副行 `{N} started outside Bunode` 或 `None right now` + 箭头 | 固定 |
| 分隔线 | 同上 | | 固定 |
| 退出确认 | 出现时插入，外边距 2/2/4，padding 10，圆角 9 | 见 §5.6 | 固定 |
| 底栏 | padding 上下 3、右 4、左 10，间距 6，高 28 | 版本文字（11.5，`label-secondary`，尾部省略）+ `Settings` + `Quit` | 固定 |

固定区合计约 156 pt（顶栏 47 + 分隔 5 + 分隔 9 + Other processes 46 + 分隔 9 + 底栏 28 + 容器内边距 12）。列表最大高度 = 屏幕 `visibleFrame` 高度 − 40 − 156，最小 200。

**Service 行**（行宽 328，圆角 9，padding-right 6，悬停背景 `fill-hover`）

```
|8|  32  |10|          238（弹性）          |6|  28  |6|
    tile     line 1: command  [:3000]          tail
             line 2: path · Status
```

- **主按钮**（`flex:1`，宽 294，高 46，padding 0/6/0/8，间距 10）：色块 → 文字列。主按钮和行尾区是两个独立点击目标。
- **色块** 32×32，圆角 9，配色和图标见 §5.1。
- **第一行**：13 pt medium，单行尾部省略，`label`。包管理器跑常用脚本时写 `{Project 名} {脚本名}`，其余整条显示、不带 Project 名，tooltip 始终是完整命令（2026-11-09 定）：Project 名是用户起的别名，没有别名时是文件夹名。常用脚本的判定：

  `/^(bun|npm|pnpm|yarn)(\s+run)?\s+((dev|start|serve|preview|build|test|lint)([:-]\S+)?)$/`

  常用脚本分三类：服务 `dev` `start` `serve` `preview`，构建 `build`，检查 `test` `lint`，带 `:` 或 `-` 后缀的算同类（`build:web`）。其他脚本名、带参数的命令（`bun run dev --port 3000`）和 `pnpm --filter` 没有预设，整条显示。

- **端口胶囊**：距命令 6，高 16，padding 0/5，圆角 4，`tag-bg`，10.5 pt 等宽数字，`label-secondary`（#48484A）。只在 Run 存活且有端口时显示（待拍板，§11）。多端口保留 `+N`，tooltip 列全部端口。胶囊不压缩，命令先省略。
- **第二行**：11.5 pt `label-secondary`，`{path}` 接 ` · {状态}`。`{path}` 是 Service 运行的目录（Project 路径，有 `folder` 时含子目录），`~` 缩写。Stopped 不显示状态词，短时 Service 的 Done 要显示（§5.1）。
- **行尾** 28×28：`rest` 箭头（10 pt，stroke 2.2，`label-tertiary`）和启停按钮（28×28，圆角 7，图标 10 pt，悬停黑 8%）叠放，用 opacity 切换，避免行宽跳动。
- **端口冲突条**挂在该行下方：外边距 2/8/6/50，左缘 50 对齐文字列（`row-text-inset`），让人看出它属于上面那行。padding 8/10，圆角 8，见 §5.3。
- 列表顺序：Project 登记顺序 + Service 创建顺序，同 Project 相邻。Agent Server 行排在所属 Project 的 Service 之后，按 `launch.json` 里的配置顺序。

**分组模式**（General › `List` 选 `By Project`，2026-11-10 定，参考 Llama app 的小标题）

```
│  Bunode ●                      Add Service │
│  2 running · 3 Services                    │
│ ─────────────────────────────────────────  │
│  api                                       │  Project 小标题: 高 24
│ [▣] dev  :3000                          ›  │  组内不写 Project 名
│     ~/code/api · Running                   │
│ [▣] bun run worker                      ›  │
│     ~/code/api                             │
│ ─────────────────────────────────────────  │  组间分隔线
│  acme                                      │
│ [▣] web  :3001                             │  Agent Server 行（launch.json 的 name）
│     ~/code/acme · Running                  │
```

- 小标题：高 24，padding 上 6 下 2，左缘 8（与色块左缘对齐），11 pt semibold，`label-secondary`，单行尾部省略。只写 Project 名（有别名用别名），不带数量和按钮。
- 组间 1 pt `separator`，左右缩进 8，上下边距 4。第一组上方不加（顶栏下已有分隔线）。
- 组不能折叠。没有任何行的 Project 不显示小标题。没有批量启停。
- 组内行标题去掉 Project 名：常用脚本只写脚本名（`dev`，不是 `api dev`），其余命令本来就整条显示，不变。tooltip、VoiceOver、第二行不变。
- 小标题和行一起在列表区滚动，不吸顶。滚动上限的公式不变。
- 平铺模式与原来相同。切换后 Popover 立即重排，不重开。

**Agent Server 行**（需求 §3.10，2026-11-10 定）

agent（首版只有 Claude 桌面 App）按 Project 根目录的 `.claude/launch.json` 启动的开发服务。只在运行时出现，外观与 Service 行相同，区别只在下面几条：

- 色块：运行中 `tile-running` + 终端提示符；Stop 后到退出前 `tile-busy` + spinner，第二行 `{path} · Stopping…`。进程退出后这一行消失。
- 第一行：配置的 `name`，两种模式都不加 Project 名。端口胶囊取该进程组实际监听的端口，规则同 Service 行。
- 第二行：`{path} · Running`，`{path}` 是 Project 根目录。
- 行尾：静止时为空（没有可进入的视图，不放箭头）。悬停或键盘聚焦时出现 Stop。
- 主按钮点击无动作，悬停底色照常（让 Stop 可发现）。
- tooltip：`name`、完整命令（`runtimeExecutable` + `runtimeArgs`）、目录，最后一行 `Started by Claude`（写具体的 agent 名）。没有徽标。
- 不计入顶栏的 running 和 Services 数量、菜单栏计数和退出确认。退出 Bunode 不停止它们。
- 不出现在 Other processes。
- Settings › Projects 的 agent 开关关闭时，这些行全部消失。

**Service 视图**（点 Service 行进入）

- 顶栏（padding 0/2/8/0，间距 6）：Back 图标按钮 28×28 → 标题块（命令，规则同 Service 行第一行，mono 12.5 semibold，单行省略；副标题 11 pt，间距 5：`{path}`、6 pt 状态点、状态词、运行时 `:port` 链接）→ `Stop`（次要按钮）或 `Start`（主按钮）。画布去掉了副标题的 `Run #N`，只留在 Logs 视图。
- 端口冲突提示（如有，外边距 0/2/10，见 §5.3）。
- 可滚动内容，三张卡片（卡片：左右外边距 2，下 10，圆角 10，背景 `card-bg`；卡片内行：最小高 38，padding 4/12，间距 10，行间 1 pt `separator`）：
  1. `Command`（输入框 180 宽 mono）· `Folder`（路径 11 pt mono 省略 + `Change…`）· `Port`（副标题 `Checked before each start`，输入框 64 宽右对齐，placeholder `none`）
  2. `Restart if it crashes`（开关，`Pauses after 3 crashes in 60 s`）· `Run through shell`（开关，`For pipes, && and globs`，默认关，C2）· `Environment`（`2 variables` / `None · .env files are not read` + `Edit…`）
  3. `Logs` 整行按钮：右侧 `N lines`（12，`label-secondary`）+ 9 pt chevron
- “已保存，下次 Run 生效”横条（见 §5.4）：外边距 0/2/10，padding 0/4/0/10，最小高 32，圆角 8。
- `Delete Service…` 文字按钮（12 pt，`status-error-text`，外边距 2/10/6）或其内联确认卡片（外边距 0/2/6，padding 10，圆角 9）。

**Add Service 视图**（点顶栏 `Add Service`）

- 顶栏：Back + `New Service`（semibold）/ 副标题 `Runs from ~/code/acme`（路径 mono，11 pt）。
- 卡片 1（padding 10/12）：`Project`（11 pt）+ Project 胶囊（高 24，左右 9，圆角 12，12 pt，选中 `accent` 底白字，未选 `Color.primary.opacity(0.08)`，不用白底）；`Scripts in package.json`（11 pt）+ 脚本胶囊（同尺寸，11 pt mono，flex-wrap 间距 6）。只列推荐短语 `dev` `start` `serve` `preview` `build` `test` `lint`；`lint:fix`、`start:w` 这类变体和其他脚本不列，手填即可（2026-11-09 定）。一个都没有时这一段不显示。
- 卡片 2：`Command`（输入框 200 宽 mono，placeholder `bun run dev`）。
- 底部右对齐，间距 6：`Cancel`（次要）· `Add`（主按钮，命令 trim 后为空时 disabled，opacity .4）。
- 默认：第一个 Project，它第一个未使用的推荐脚本，命令按 §5.2 生成。新 Service 默认无 Port、`Run through shell` 关、`Restart if it crashes` 取 General 默认（C3）。
- Service 名不再手填：取脚本名，手填命令时取最后一个词。冲突处理见 §6.2。

**Logs 视图**（Service 视图里点 Logs）——**已取消**（2026-11-11）：Service 视图的 Logs 行尾是 `arrow.up.forward.app` 图标，点击在控制台中打开日志文件，tooltip `Open in Console`；没有文件时 toast `No log file yet.`（[ADR-0008](adr/0008-logs-open-in-console.md)）。以下保留作历史记录。

- 顶栏：Back（返回 Service 视图，不是 Home）+ 标题 / 副标题 `Logs · Run #14` 或 `Logs · No Runs yet` + `Restart`（运行中）或 `Start`（主按钮）。
- 搜索框：高 28，圆角 7，背景 `fill-subtle`，左 13 pt 放大镜，placeholder `Search`。外边距 0/2/6。
- 日志区：**AppKit 虚拟化视图**（`NSTableView`/`NSTextView` + 按可见范围拉取，需求 §2.2），画布高 400 pt；背景 `log-bg`（深色，浅色模式下也保持深色）；圆角 9；上下 padding 8；11 pt mono，行高 1.6。每行两列：时间（58 pt 宽，`HH:mm:ss`，`log-time`）+ 正文（自动换行、长单词断行）。列间距 8，行左右 padding 10。
- 底栏（padding 8/2/2，间距 4）：左侧 `N lines` 或 `N lines · M matches`（11 pt）· `Clear` · `Copy`（透明次要按钮）· `Export…`（`NSSavePanel`，导出该 Run 的完整日志文件，默认文件名 `api-dev-run14.log`）。丢弃过行时，左侧计数后加 ` · 1,204 dropped`（同为 `label-secondary`）。

**Environment 视图**（Service 视图里点 `Environment › Edit…`，新画布未单独画，沿用上一版）

- 顶栏：Back（返回 Service 视图）+ `Environment` / 副标题 Service 全名。
- 一张卡片，每行一个变量：Key 输入框（12 pt mono，110 宽）· `=` · Value 输入框（12 pt mono，`flex:1`，默认遮罩为 `••••••`）· 显示 / 隐藏图标按钮（`eye` / `eye.slash`，24×24）· 删除图标按钮（`minus.circle`，24×24）。
- 卡片最后一行：`Add Variable` 文字按钮（`accent`），新增空行并聚焦 Key。
- 卡片下方 11 pt 说明：`.env files are not read. Your tools load them as usual.`
- 自动保存（失焦或回车），规则同 §5.4：Run 存活时出现 `Saved. Applies on the next Run.` 条。
- 校验：Key 只允许 `[A-Za-z_][A-Za-z0-9_]*`，同一 Service 内唯一；不合法时 Key 框红色描边，下方 11 pt `Invalid name` / `Already set`，该行不保存。Key 和 Value 都为空的行在离开视图时丢弃。
- 空态：卡片内只有 `Add Variable` 一行。
- VoiceOver：Value 框 label `Value of PORT`，遮罩时读作 `hidden`。

**Other processes 视图**（清单下方的 Other processes 行，或冲突提示的 `Show Process`）

- 顶栏：Back + `Other processes` / 副标题 `Started outside Bunode · watch only`。
- 一张卡片，每行：命令（13 pt mono 单行省略）+ 副标题 `Terminal · PID 48121 · :4321`（来源 · PID · 端口，无端口则省略）+ `Stop…`。
- 空态：居中 `No other Node or Bun processes.`，上下 padding 30。

### 2.3 Settings 窗口

| 项 | 规格 |
|---|---|
| 窗口 | 标准 `NSWindow`（`Settings` scene），内容固定 **700×540 pt**（画布尺寸，刻意偏小，不再放大），不可调整大小；Projects 很多时主区内滚动 |
| 侧栏 | 宽 168，背景 `sidebar-bg`，padding 14/8/12；交通灯下方留 26。导航项高 32，圆角 8，图标 17 pt + 9 间距 + 13 pt medium 文字。底部 `Source code` 外链（尾部 11 pt ↗） |
| 选中态 | 背景 `accent`、文字白色；落地用 `List(selection:)` + `.sidebar` 样式，系统会给出正确选中色 |
| 主区 | 标题 15 pt bold，padding 20/22/14；内容 padding 0/20/20，可滚动，分组间距 12 |
| 分组卡片 | 圆角 10，背景 `grouped-bg`；行最小高 52，padding 8/10，左右外边距 2，行间 1 pt `separator`；左侧标签（13 pt）+ 帮助（11 pt `secondaryLabel`，单行省略），右侧控件。落地用 `Form { Section {} }.formStyle(.grouped)` |

**General**

| 分组 | 行 | 控件 | 默认 |
|---|---|---|---|
| App | Open at login · `Bunode lives in the menu bar.` | Switch | **Off**（首次启动不自动注册登录项，由用户自己打开） |
| App | Show running count · `Next to the menu bar icon.` | Switch | On |
| Runs | Restart Services that crash · `Up to 3 times a minute, then it waits for you.` | Switch | On（C3：只作新 Service 的默认值） |
| Runs | Notify on crash · `One notification per crash.` | Switch | On |
| Display | List · `How the menu bar list is arranged.` | `Picker(.segmented)`：`Flat` / `By Project` | Flat |
| Display | Language · `Applies after Bunode restarts.` | `Picker(.menu)`：`System` / `English` / `简体中文` | System |

`Language`（2026-11-10 定，[ADR-0006](adr/0006-language-switch-needs-restart.md)）：

- `English` 和 `简体中文` 两项永远用各自的语言写，不翻译；`System` 跟当前界面语言（中文 `跟随系统`）。
- 选择后写入本 App 的 `AppleLanguages`（`System` 删除这个键），界面不立即变。
- 选中的语言和当前界面语言不同时，帮助文字换成 `Restart Bunode to apply.`，行右侧控件前多一个 `Restart Now` 按钮（`.bordered`，`.controlSize(.small)`）。选回当前语言时恢复原样。
- `Restart Now`：没有 Run 时直接退出并重新打开。有 Run 时先弹 §2.4 的重启确认，所有 Run 停止后再退出（ADR-0003）。重新打开由退出前派生的 `/bin/sh -c` 完成：约一秒后等 Bunode 进程消失，再 `open` 本 App 的路径（不用 bundle id，免得打开另一份拷贝）。它不是 Run，不受 lifeline 约束。

**Projects**

- 一张卡片，每行：名字（13 pt）+ 副标题 `~/code/api · Bun · 2 Services`（路径 mono）+ `Remove…`。
- 名字默认是文件夹名。点击变成输入框（`.roundedBorder`，`.controlSize(.small)`，最宽 220），可填别名，占位符是文件夹名；回车或失焦保存，Esc 放弃，留空回到文件夹名。别名替换所有显示 Project 名的地方。
- 包管理器（C6，待拍板，§11）：默认只在检测到多个锁文件时显示弹出菜单（`Auto (Bun)` / `npm` / `pnpm` / `yarn` / `Bun`，`.controlSize(.small)`）和 `exclamationmark.triangle`（`systemOrange`）图标，tooltip / VoiceOver：`Several lockfiles found. Pick a package manager.`。改包管理器只影响之后新建的 Service 生成的命令和下一次 Run。
- 卡片下方：左侧说明 `Services are added and set up from the menu bar.`（11 pt）+ 右侧 `Add Project…`（打开 `NSOpenPanel`，只能选文件夹）。
- 再下方一张单行卡片：`Show dev servers started by agents` + Switch，默认 On。帮助文字 `Servers an agent such as Claude starts from its launch settings in a Project's folder. You can stop them here.` 较长，这一行的帮助允许换成两行，不省略。中文见 §6.6。关闭后不再读 agent 配置，Home 里的 Agent Server 行全部消失，这些进程如果是 Node/Bun，回到 Other processes。
- 空态：卡片内居中 `No Projects yet.`。

**Advanced**

| 分组 | 行 | 右侧 | 行为 |
|---|---|---|---|
| Env | Shell environment · `87 variables from /bin/zsh · today 09:58` | `Reload` 按钮 | 点击后按钮变 `Reloading` + spinner 并 disabled；完成后帮助文字更新为 `just now`，Toast `Environment reloaded. New Runs get it.` |
| Env | Node.js · 路径（mono） | 版本 `22.11.0` + `Change…` | `NSOpenPanel` 选可执行文件 |
| Env | Bun · 路径（mono） | 版本 `1.2.4` + `Change…` | 同上 |
| Stop | Force stop after · `Stop asks nicely first, then force-quits.` | 弹出菜单 `5 s` | 选项 3 / 5 / 10 / 30 s，默认 5（需求 §3.2-7） |
| Stop | Log lines kept · `Per Service. Export saves the full log.` | 弹出菜单 `10,000` | 选项 5,000 / 10,000 / 50,000，默认 10,000 |
| Stop | Diagnostics · `Supervisor and runner logs.` | `Open` | Finder 打开 `~/Library/Logs/Bunode` |

> 画布里弹出菜单是“点一下轮换下一个值”的假控件，落地必须是真正的 `Picker(.menu)`。

### 2.4 确认对话框

用系统 `NSAlert`（或 `.confirmationDialog`/`.alert`），不要自绘。画布尺寸仅作参考：宽 300，圆角 18，App 图标 56，标题 14 bold，正文 12，按钮纵向堆叠高 30、间距 8（这正是 macOS 26 alert 的系统样式）。

| 对话框 | 标题 | 正文 | 主按钮 | 次按钮 |
|---|---|---|---|---|
| 退出（有 Run） | `Quit Bunode?` | `2 Services are running. Bunode stops every Run before it quits, so it never leaves processes behind.` | `Stop 2 Services and Quit`（默认，Return） | `Cancel`（Esc） |
| 切换语言后重启（有 Run） | `Restart Bunode?` | `2 Services are running. Bunode stops every Run before it restarts.` | `Stop 2 Services and Restart`（默认，Return） | `Cancel`（Esc） |
| 删除 Project（有运行中的 Service） | `Remove “api”?` | `Its 2 Services leave Bunode. Files in ~/code/api stay on disk. 1 running Service will be stopped first.` | `Stop 1 and Remove`（destructive） | `Cancel`（默认） |
| 强制结束外部进程 | `Force kill node (PID 51207)?` | `Bunode didn’t start this process. It did not exit 5 s after SIGTERM. Force killing it may lose unsaved work.` + mono 信息框 `next dev -p 3001 · ~/code/landing · started from VS Code` | `Force Kill`（destructive） | `Keep Running`（默认） |

> 画布 Dialogs 板用的是 `Delete “api”?`，Settings 板用的是 `Remove`。**统一用 Remove**（Project 是“从 Bunode 移除登记”，Delete 容易被读成删文件）；Service 层面沿用 `Delete Service…`（它只存在于 Bunode 里）。

### 2.5 尺寸适应

固定宽度的 macOS 菜单栏工具，没有响应式断点。

| 情况 | 变化 |
|---|---|
| Service 少 | Popover 贴合内容高度，不留空白 |
| Service 多或屏幕矮 | 只有 Service 列表滚动，顶栏、Other processes、底栏固定 |
| 子视图内容超高 | 内容区滚动，顶栏固定 |
| Settings 窗口 | 700×540 不可拉伸 |
| 深色模式〔设计未画〕 | 跟随系统，靠 §3 的语义色和 Asset Color |
| 增强对比度 | 系统色自动适应，Asset Color 需要提供高对比变体 |

---

## 3. 设计 Token

画布没有接设计系统，颜色是写死的 hex。下表给每个值起名，并映射到 macOS 语义色——**代码里只用右列**，这样深色模式（需求 §4.1 跟随系统）自动成立。

### 3.1 颜色

| Token | 画布值（浅色） | SwiftUI / AppKit | 用途 |
|---|---|---|---|
| `label` | `#1D1D1F` | `.primary` / `labelColor` | 命令、标题 |
| `label-secondary` | `#6C6C70`、`#48484A` | `.secondary` / `secondaryLabelColor` | 第二行、帮助文字 |
| `label-tertiary` | `#AEAEB2`、`#8A8A8E` | `tertiaryLabelColor` | 行尾箭头（装饰） |
| `accent` | `#0A63D6` | `Color.accentColor` / `controlAccentColor` | 主按钮、`Add Service`、链接、开关开 |
| `separator` | `rgba(0,0,0,.07–.08)` | `separatorColor` | 行分隔、Popover 分隔线 |
| `fill-hover` | `rgba(0,0,0,.05)` | `Color.primary.opacity(0.07)` | 行和按钮悬停、Service 行键盘选中 |
| `fill-subtle` | `rgba(0,0,0,.05–.06)` | `.quinary` / `controlBackgroundColor` | 搜索框、次要按钮底 |
| `card-bg` | `rgba(0,0,0,.05)` | `.quinary` | Popover 卡片，半透明，不用白底（2026-11-09 定） |
| `tag-bg` | `rgba(0,0,0,.06)` | `.quinary` | 端口胶囊 |
| `tile-stopped-bg` / `-fg` | `rgba(0,0,0,.07)` / `#6C6C70` | `.quaternary` / `.secondary` | Stopped 色块 |
| `tile-running-bg` / `-fg` | `#DDF5E3` / `#1B7F3A` | Asset Color（含深色） | Running 色块 |
| `tile-busy-bg` / `-fg` | `#FFEFD6` / `#9A5B00` | Asset Color（含深色） | Starting / Stopping / Restarting |
| `tile-error-bg` / `-fg` | `#FFE3E0` / `#B42318` | Asset Color（含深色） | Crashed / Failed |
| `grouped-bg` | `#F5F5F7` | `Form(.grouped)` 自带 | Settings 分组 |
| `sidebar-bg` | `#EDEDF0` | `.sidebar` 列表样式自带 | Settings 侧栏 |
| `status-running` | `#34C759` | `.green` / `systemGreen` | 菜单栏、顶栏、Service 视图状态点 |
| `status-busy` | `#FF9F0A` | `.orange` / `systemOrange` | Service 视图状态点（脉冲） |
| `status-error` | `#FF3B30` | `.red` / `systemRed` | 菜单栏、顶栏、Service 视图状态点 |
| `status-idle` | `#AEAEB2` | `systemGray`（`.gray`） | Stopped 状态点 |
| `status-error-text` | `#B42318` | `systemRed` 文本变体（深色下要亮） | 第二行 Crashed、`Delete Service…`、校验错误 |
| `destructive` | `#D70015`、`#C4000F`（两处不一致） | `.destructive` 按钮角色 | 删除 / 强杀确认按钮。**统一为系统 destructive，不选 hex** |
| `warning-bg` / `warning-text` | `#FFF4E0` / `#5C3A00` | 自定义 Asset Color（含深色变体） | 端口冲突、SIGTERM 确认内联条 |
| `info-bg` / `info-text` | `rgba(10,99,214,.08)` / `#0A4FA8` | `accentColor.opacity(0.1)` / `accentColor` | “Applies on the next Run” 条 |
| `log-bg` | `#1C1C1E` | 自定义 Asset Color，两种外观都用深色 | 日志区背景 |
| `log-text` / `log-time` / `log-sys` / `log-err` | `#E5E5EA` / `#8E8E93` / `#98989D` / `#FF6961` | 自定义 Asset Color | stdout / 时间戳 / Bunode 系统行 / stderr。ANSI 颜色覆盖 `log-text` |
| `toast-bg` | `rgba(29,29,31,.92)` | 自定义（或 HUD 材质） | Toast |

色块深色变体〔设计未画〕：bg 用对应系统色 22% 透明度，fg 用系统色本身，`status-error-text` 用 `systemRed`。

### 3.2 字体（系统字体 SF Pro / SF Mono）

| Token | 规格 | SwiftUI | 用途 |
|---|---|---|---|
| `font-body` | 13 / regular / 行高 1.35 | `.body`（macOS 13 pt） | 正文、卡片行标题 |
| `font-body-strong` | 13 / semibold（画布 650） | `.body.weight(.semibold)` | Popover 视图标题 |
| `font-row-title` | 13 / medium | `.body.weight(.medium)` | Service 行第一行 |
| `font-row-sub` | 11.5 / regular | `.system(size: 11.5)` | Service 行第二行、版本、底栏按钮 |
| `font-tag` | 10.5 / 等宽数字 | `.system(size: 10.5).monospacedDigit()` | 端口胶囊 |
| `font-app-title` | 14 / semibold | `.system(size: 14, weight: .semibold)` | 顶栏 `Bunode` |
| `font-link` | 12.5 / regular | `.system(size: 12.5)` | `Add Service` |
| `font-callout` | 12 / regular | `.callout` | 顶栏副行、按钮、提示条、Toast |
| `font-caption` | 11 / regular | `.caption` | 帮助文字、子视图副标题 |
| `font-mono` | 11–12.5 / SF Mono | `.system(.caption, design: .monospaced)` | 路径、PID、日志、Service 视图命令 |
| `font-title` | 15 / bold | `.title3.bold()` | Settings 页标题 |
| `font-alert-title` | 14 / bold | `NSAlert` 自带 | 对话框标题 |

半磅字号有意保留：第二行要明显弱于第一行，但 11 pt 在两行紧贴时偏小。

### 3.3 间距与圆角

| Token | 值 | 用途 |
|---|---|---|
| `space-2xs` | 2 | 卡片左右外边距、提示条内缩 |
| `space-xs` | 4 | 图标与数字、行纵向 padding |
| `space-sm` | 6 | Popover 内边距、按钮组间距、胶囊间距 |
| `space-md` | 8 | 行内元素间距、视图顶栏下边距 |
| `space-lg` | 10 | 卡片行横向 padding、卡片间距、色块与文字列间距 |
| `space-xl` | 12 | 卡片行 padding、Settings 分组间距 |
| `space-2xl` | 20 | Settings 主区 padding |
| `row-height-two-line` | 46 | Service 行、Other processes 行 |
| `row-text-inset` | 50 | 文字列左缘（8 + 32 + 10），行下附属内容对齐到这里 |
| `radius-xs` | 5 | 菜单栏项高亮 |
| `radius-sm` | 6 | 按钮、输入框 |
| `radius-md` | 7–8 | 图标按钮、提示条、导航项 |
| `radius-lg` | 9–10 | Service 行、色块、内联确认卡片、内容卡片、日志区 |
| `radius-pill` | 12 | Project 和脚本胶囊 |
| `radius-popover` | 14 | Popover（系统） |

### 3.4 控件尺寸

| 控件 | 尺寸 | 落地 |
|---|---|---|
| 主 / 次按钮 | 高 26，左右 12 / 10，12 pt | `.controlSize(.small)` + `.borderedProminent` / `.bordered` |
| 底栏按钮 | 高 22，左右 8，描边 1 pt 黑 16%，圆角 6，11.5 pt | `.bordered` + `.controlSize(.mini)`，外观不符时自定义 ButtonStyle |
| 图标按钮 | 28×28 | `.buttonStyle(.borderless)` + `.help()` |
| 状态色块 | 32×32，圆角 9 | 自绘 `RoundedRectangle` |
| 开关 | 34×20，滑块 16 | `Toggle().toggleStyle(.switch).controlSize(.small)` |
| 输入框 | 高 24（Popover）/ 26（Settings），12 pt | `TextField().controlSize(.small)` |
| 状态点 | 7×7（顶栏）、6×6（Service 视图副标题） | `Circle()` |

---

## 4. Components

| 组件 | 变体 | 输入 | 说明 |
|---|---|---|---|
| `StatusItemView` | — | `runningCount`, `aggregate: idle/running/error`, `showCount` | §2.1 |
| `ServiceRow` | stopped / busy / running / crashed / failed | `service`, `project`, `status`, `isNotResponding`, `open` | 主按钮打开 Service 视图。保留右键菜单（Restart、Stop、Start、Open），这是鼠标用户从 Home 重启的唯一入口 |
| `StatusTile`（新） | stopped / running / busy / error | `style` | 替代行内的 `StatusDot`，复用 `RunStatus.dotStyle` 的分类 |
| `StatusDot` | idle / running / busy（脉冲） / error | `RunStatus`, `size` | 顶栏和 Service 视图副标题 |
| `PortLabel` | tag | `service`, `status` | 胶囊外观，多端口 `+N` |
| `RowAction` | chevron / start / stop | `status`, `isNotResponding`, `isRevealed` | 箭头和按钮叠放 |
| `HomeHeader` | — | `running`, `total`, `hasError` | `Add Service` 用 `.buttonStyle(.link)` |
| `OtherProcessesRow`（新） | 有 / 无 | `count` | 外观和 Service 行相同，整行一个按钮 |
| `PopoverFooter`（新） | — | `versionText` | 替代 `MenuRow` |
| `InlineNotice` | warning / info | `text`, `actions[]` | 端口冲突、SIGTERM 确认、“下次 Run 生效” |
| `InlineConfirm` | neutral / destructive | `text`, `confirmLabel`, `onConfirm`, `onCancel` | Popover 内退出确认、删除 Service 确认。按钮右对齐：Cancel 在左 |
| `SettingsCard` / `SettingsRow` | toggle / picker / button / value+button | `label`, `help`, `control` | 用 `Form(.grouped)` 的 `LabeledContent` |
| `LogView`（AppKit） | — | `runId`, `query` | 虚拟化，按可见范围从 Rust 拉取 |
| `Toast` | — | `message` | 见 §6.4 |
| `ScriptChip` | default / selected / used(disabled) | `script`, `selected`, `used` | 已有同命令 Service 的脚本 disabled，灰字 `#8A8A8E` |
| `ProjectHeader`（新） | — | `title` | 分组模式的小标题，§2.2。`accessibilityAddTraits(.isHeader)` |
| `AgentServerRow`（新） | running / stopping | `server`, `project` | §2.2。复用 `StatusTile`、`PortLabel`，行尾只有 Stop |

Agent Server 的状态单独一组（`AgentServerState`：`project_id`、`agent`、`name`、`command`、`pid`、`pgid`、`live_ports`、`stopping`），不混进 `ServiceState`。

UI 只读 Supervisor 推导的状态（`ServiceState`：`id`、`project_id`、`name`、`display_name`、`command`、`folder`、`port`、`run_through_shell`、`status`、`run_number`、`pid`、`last_exit`、`log_*`、`has_pending_config`、`ended_by`、`live_ports`）。

---

## 5. States and Interactions

### 5.1 Service 状态 → 视觉

| 状态 | 色块（bg / fg / 图标） | 第二行 | 端口胶囊 | 悬停行尾 | Service 视图顶栏 | Logs 顶栏 |
|---|---|---|---|---|---|---|
| Stopped | `tile-stopped` / 终端提示符 16 pt | `{path}` | 无 | Start | `Stopped` + `Start` | `Start` |
| Starting | `tile-busy` / spinner 14 pt | `{path} · Starting…` | 无 | 无，箭头保留 | `Starting…`，无按钮 | 无按钮 |
| Running | `tile-running` / 终端提示符 16 pt | `{path} · Running` | 有端口时显示 | Stop | `Running` + `:port` 链接 + `Stop` | `Restart` |
| Stopping / Restarting | `tile-busy` / spinner | `{path} · Stopping…` / `Restarting…` | 无 | 无，箭头保留 | `Stopping…` / `Restarting…` | 无按钮 |
| Crashed | `tile-error` / 感叹号 15 pt | `{path} · Crashed`（`status-error-text`） | 无 | Start | `Crashed` + `Start` | `Start` |
| Failed | `tile-error` / 感叹号 15 pt | `{path} · Failed`（`status-error-text`）〔设计未画〕 | 无 | Start | `Failed` + `Start` | `Start` |
| Not responding | `tile-busy` / spinner | `{path} · Not responding`（`status-error-text`）〔设计未画〕 | 无 | Stop，再点触发 Force Kill | `Not responding` + `Stop` | 无按钮 |

图标路径（16 viewBox）：提示符 `M3.5 4.5 7 8l-3.5 3.5M8.5 12h4.5`（stroke 1.7）；spinner `M8 2a6 6 0 1 0 6 6`（stroke 2）；感叹号 `M8 3v6`（stroke 2）+ circle cx8 cy12.5 r1.2；Stop rect x3 y3 10×10 rx2；Start `M4 2.5v11l9.5-5.5z`。

行的 tooltip 显示完整命令和目录；Crashed、Failed 时再加 `last_exit`。

**短时 Service**（2026-11-09 定）：构建和检查类常用脚本（`build` `test` `lint`，§2.2）会自己结束。带 `:watch` 或 `-watch` 后缀的不算，仍是长时。服务类和没有预设的命令都按上表显示。短时 Service 与上表的差别：

| 状态 | 色块（bg / fg / 图标） | 第二行 | Service 视图顶栏 |
|---|---|---|---|
| Stopped（还没跑过，或被 Stop） | `tile-stopped` / SF `hammer` 13 pt | `{path}` | `Stopped` + `Run` |
| Running | `tile-busy` / spinner | `{path} · Running…`，无端口胶囊 | `Running…` + `Stop` |
| Stopped（自己以 0 退出） | `tile-running` / SF `checkmark` 12 pt | `{path} · Done` | `Done` + `Run` |
| Crashed | `tile-error` / 感叹号 15 pt | `{path} · Failed`（`status-error-text`） | `Failed` + `Run` |

上表其余状态不变。所有 `Start`（行尾、右键、Service 视图、Logs）都写 `Run`。短时 Service 不计入 running 数量；失败仍计入崩溃数，菜单栏点变红。

规则：

- **防重复点击**：Start 后立即切到 busy，行尾按钮消失，直到 Supervisor 推回 `running`/`failed`（需求 §3.2-3）。UI 不自己计时；原型里的 1000 / 800 / 1200 ms 只是演示。
- 100 ms 内必须有可见反馈（需求 §5.2）：本地乐观地把色块切到 busy，真实状态以 Supervisor 事件为准。
- 后端超时（需求 §2.2-8）：Starting/Stopping 超过 Force stop 时长 + 5 s 未收到事件 → `Not responding`，按钮恢复，可再点 Stop 触发 Force Kill。

### 5.2 元素交互

| 元素 | 状态 | 行为 |
|---|---|---|
| 菜单栏图标 | 点击 | 开 / 关 Popover。再次打开时总是回到 `home` 视图 |
| Service 行 | Hover | 背景 `fill-hover`，100 ms 过渡。箭头淡出，Start 或 Stop 淡入。busy 状态没有按钮，箭头不变 |
| Service 行 | 键盘聚焦 | 同 Hover，不画焦点环，启停按钮可见 |
| Service 行 | 点击 | 进入 Service 视图 |
| Service 行 | 右键 | Running：Restart、Stop、Open。Stopped、Crashed：Start、Open。busy：Open |
| Agent Server 行 | Hover / 键盘聚焦 | 同 Service 行，行尾出现 Stop |
| Agent Server 行 | 点击 | 无动作 |
| Agent Server 行 | 右键 | `Stop` · `Open in Browser`（有端口时，打开 `http://localhost:<port>`）· `Show in Finder`（Project 根目录）。stopping 时只有后两项 |
| Agent Server 的 Stop | 点击 | 不确认。向启动器进程组发 SIGTERM，色块转 busy；超过 Force stop 时长仍未退出则自动 SIGKILL。退出后行消失。SIGKILL 后 1 s 仍存活：行回到 Running，Toast `Couldn’t stop web.` |
| General › `List` | 切换 | Popover 立即按新模式重排 |
| General › `Language` | 选择 | 见 §2.3 |
| Projects › agent 开关 | 切换 | 立即生效 |
| Start / Stop 图标 | 点击 | 直接启停，不确认，不触发行点击 |
| `Add Service` | 点击 | 进入 Add Service，默认第一个 Project 和它第一个“未被使用”的脚本，命令按包管理器生成：Bun `bun run x`、pnpm `pnpm x`、npm `npm run x`、yarn `yarn x` |
| Project 胶囊 | 点击 | 切换 Project，脚本和命令按新 Project 重新生成 |
| 脚本胶囊 | 点击 | 选中，同时填好 Command；之后手改 Command 会取消胶囊选中 |
| `Add` | 可用 | 命令非空。成功后回 Home，Toast `Added acme › storybook` |
| `Add` | Disabled | opacity .4，无 hover |
| 端口链接 | 点击 | 浏览器打开 `http://localhost:<livePort>`（需求 §3.5） |
| `Folder › Change…` | 点击 | `NSOpenPanel`，起始目录为 Project 根，**只允许选 Project 目录或其子目录**（需求 §3.1），选到外面时提示并拒绝 |
| `Port` 输入 | 输入 | 只接受数字，最多 5 位，1–65535；空 = 无预期端口 |
| `Environment › Edit…` | 点击 | 进入 Environment 视图（§2.2）。值默认遮罩（需求 §6） |
| `Logs` 行 | 点击 | 进入 Logs 视图，清空搜索词 |
| `Delete Service…` | 点击 | 原位替换为 `InlineConfirm`（见 §5.5） |
| Other processes 行 | 点击 | 进入 Other processes |
| `Stop…`（外部进程） | 点击 | 行下展开 warning 条 `Send SIGTERM to PID 51207? Bunode didn’t start it and can’t bring it back.` + `Cancel` / `Send SIGTERM`（`label` 色深底按钮） |
| `Send SIGTERM` | 确认 | 行变 opacity .45 + `Exiting…`；进程退出后从列表移除。**5 s 后仍存活 → 弹 §2.4 Force kill 对话框** |
| `Settings` | 点击 / ⌘, | 打开或前置 Settings 窗口，关闭 Popover |
| `Quit` | 点击 / ⌘Q | 无 Run → 直接退出；有 Run → 确认（见 §5.6） |

### 5.3 端口冲突（启动前检查）

触发：Start 一个配置了 `Port` 的 Service，而该端口被占用。

- 不启动，色块回到 Stopped。在该行下方（Home）或顶栏下方（Service 视图）插入 warning 条：
  `Port 4321 is used by “astro dev” from Terminal. Bunode won’t stop it.`，占用者未知时 `Port 4321 is used by another program. Bunode won’t stop it.`
- 按钮：`Use :4322`（主，没有可用建议时不显示）· `Show Process`（白底次要，跳 Other processes，占用者是另一个 Service 时不显示）· `Cancel`（透明）。
- `Use :N` 的 N = 从配置端口 +1 起第一个空闲端口（画布只是 +1）。
- 端口传递：只为本次 Run 设置环境变量 `PORT=4322`，不改写命令参数（需求 §3.5）。条内正文下方加一行 11 pt：`Sets PORT=4322 for this Run. Tools that ignore PORT need their own flag.` Service 的 Port 配置不变，下一次 Run 仍先检查原端口。
- 占用者是 Agent Server 时：`Port 3001 is used by “web” from Claude. Bunode won’t stop it.`，不显示 `Show Process`（它不在 Other processes），用户可以在它自己的行上 Stop。
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

- Popover 内 `Quit`：底栏上方插入 `InlineConfirm`：`2 Services are running. Quitting stops every Run.` + `Cancel` / `Stop 2 Services and Quit`。确认后换成 spinner `Stopping all Runs…`，全部停止后退出。画布的按钮是 `Stop N and Quit`，结束画面 `Bunode quit` 只是原型演示。
- 其他入口的 ⌘Q（Settings 窗口在前时、Dock 菜单、系统注销）：用 §2.4 的 `NSAlert`。
- 两种确认都保留：Popover 打开时用内联，其余入口用 alert，按钮文案统一为 `Stop N Services and Quit`。
- 系统关机 / 注销（`applicationShouldTerminate` 来自 `NSWorkspace` power-off）：不弹确认，直接停止所有 Run（需求 §3.8 区分 OS 重启）。
- 切换语言后的 `Restart Now` 走同一套流程，文案换成 Restart（§2.3、§2.4）。
- 退出时 Agent Server 不停止，也不计入 N。

### 5.7 Settings 交互

| 元素 | 行为 |
|---|---|
| 侧栏项 | 切换页面，关闭任何打开的 sheet |
| 开关 | 立即生效并持久化；150 ms 过渡 |
| `Remove…`（Project） | 弹 §2.4 alert（画布是窗口内 sheet：340 宽、距顶 120、背景遮罩 `rgba(0,0,0,.16)` —— 用系统 sheet 即可）。无 Run 时按钮 `Remove`，有 Run 时 `Stop N and Remove` |
| Remove 确认后 | 先停 Run，再删登记 → Toast `Removed api. Files untouched.` |
| `Add Project…` | `NSOpenPanel` 选文件夹 → 识别包管理器 → 加入列表，新行的名字直接进入输入状态 → Toast `Added ~/code/invoice-bot (Bun). Add its Services from the menu bar.`；已登记 → Toast `invoice-bot is already added` |
| `Source code` | 浏览器打开 `https://github.com/imal1/bunode` |

---

## 6. 内容规范

### 6.1 截断

| 文本 | 规则 |
|---|---|
| 行第一行（命令） | 单行尾部省略，tooltip 显示完整命令。端口胶囊优先保留 |
| 行第二行 | 单行中间省略。状态词不被挤掉，路径先省略 |
| Service 视图标题 / Logs 标题 | 单行尾部省略。全名放进 tooltip（`.help`）与 VoiceOver |
| 路径（Folder、Projects 行、Node/Bun 路径） | 单行，**中间省略**（`truncationMode(.middle)`），保留末段目录名 —— 画布是尾部省略，换成中间更有用 |
| 外部进程命令 | 单行尾部省略，tooltip 显示完整命令 |
| 帮助文字 | 单行尾部省略（画布如此）。中文本地化后可能过长，见 §7 |
| 版本文字 | 尾部省略，`Settings`、`Quit` 按钮不压缩 |
| 按钮文案 | 不截断，允许按钮变宽 |
| 日志行 | 自动换行，长 token 任意断行 |
| 对话框 meta 框 | 自动换行，任意断行 |

### 6.2 字数与校验

| 字段 | 限制 |
|---|---|
| Service Name | 1–40 字符，同一 Project 的同一目录内唯一（不分大小写）。本版不再手填：自动取名超过 40 字符时截断；已被占用时 `Add` disabled，Command 框下 11 pt 红字 `Name already used in acme`〔设计未画〕 |
| Command | 非空，不限长度，单行输入框横向滚动 |
| Port | 1–65535 |

### 6.3 空态

| 位置 | 文案 |
|---|---|
| Home 无 Project〔设计未画〕 | 居中：`No Projects yet.` + 主按钮 `Add Project…`（打开 Settings › Projects 并触发选择）。隐藏顶栏 `Add Service`，因为 Service 必须属于 Project |
| Home 有 Project 无 Service | 列表区 padding 30/16 居中：`No commands yet.`（12，`label-secondary`） |
| Other processes 行为空 | 副行 `None right now`，行仍显示 |
| Logs 无输出 | `No output yet.` |
| Logs 搜索无结果 | `No lines match.` |
| Other processes 视图为空 | `No other Node or Bun processes.` |
| Settings Projects 为空 | `No Projects yet.` |
| Add Service 无 package.json〔设计未画〕 | 隐藏脚本胶囊，Command 直接聚焦 |
| Add Service 脚本全部已用 | 全部灰显，Command 留空待手填 |

### 6.4 Toast

- 位置：所在窗口底部居中，距底 16–18；padding 7–8/14；圆角 9–10；最大宽 380（Popover）/ 460（Settings）。
- 2.4 s 后自动消失；新 Toast 替换旧的。`role=status`（VoiceOver 播报，见 §9）。
- 文案见 §5。原型里“Opens a file picker”之类 Toast 只是占位，真实行为替代它们。

### 6.5 数字格式

- 端口 `:3000`（冒号前缀）。Run 号 `Run #14`。日志计数 `250 lines`、单复数正确（`1 line`、`1 Service`）。
- `10,000` 用系统 `NumberFormatter`（随 locale）。时间戳 `HH:mm:ss` 24 小时。
- 底栏版本：`{App 版本} · Bun {版本} · Node {版本}`，值取 Advanced 里的运行时版本。缺失时省略对应一段〔设计未画〕。画布的 `0.1.0 · Bun 1.2.4 · Node 22.11` 是演示值。

---

### 6.6 简体中文（2026-11-10 定）

- 固定译名以 `GLOSSARY.md` 为准：Project 项目、Service 服务、Run 运行、Other processes 其他进程、短时 Service 一次性服务。日志内容、命令、路径、端口、PID 不翻译。
- 中文与英文、数字之间加半角空格（`3 个服务`、`Bun 1.2.4`）。用全角标点，命令和路径里的标点照原样。
- 中文普遍比英文短，布局按英文定。按钮仍然不截断。

主要文案：

| English | 简体中文 |
|---|---|
| `%d running · %d Services` | `%d 个运行中 · %d 个服务` |
| `Add Service` · `Settings` · `Quit` | `添加服务` · `设置` · `退出` |
| `Start` · `Stop` · `Restart` · `Run` | `启动` · `停止` · `重启` · `运行` |
| `Running` · `Starting…` · `Stopping…` · `Restarting…` · `Stopped` | `运行中` · `正在启动…` · `正在停止…` · `正在重启…` · `已停止` |
| `Crashed` · `Failed` · `Not responding` · `Folder missing` | `已崩溃` · `失败` · `无响应` · `目录不存在` |
| `Running…` · `Done`（一次性服务） | `运行中…` · `已完成` |
| `Other processes` · `%d started outside Bunode` · `None right now` | `其他进程` · `%d 个在 Bunode 外启动` · `暂无` |
| `Started by %@` | `由 %@ 启动` |
| `Open in Browser` · `Show in Finder` | `在浏览器中打开` · `在访达中显示` |
| `General` · `Projects` · `Advanced` | `通用` · `项目` · `高级` |
| `List` · `How the menu bar list is arranged.` · `Flat` · `By Project` | `列表` · `菜单栏列表的排列方式。` · `平铺` · `按项目` |
| `Language` · `Applies after Bunode restarts.` · `System` | `语言` · `重启 Bunode 后生效。` · `跟随系统` |
| `Restart Bunode to apply.` · `Restart Now` | `重启 Bunode 后生效。` · `立即重启` |
| `Show dev servers started by agents` | `显示 agent 启动的开发服务` |
| `Servers an agent such as Claude starts from its launch settings in a Project's folder. You can stop them here.` | `由 Claude 等 agent 按其启动配置在项目目录中运行的服务，可在此停止。` |
| `Restart Bunode?` · `Stop %d Services and Restart` | `重启 Bunode？` · `停止 %d 个服务并重启` |
| `Quit Bunode?` · `Stop %d Services and Quit` | `退出 Bunode？` · `停止 %d 个服务并退出` |

其余文案在 String Catalog 里按本节规则翻译。

---

## 7. 边界情况

- **30+ Service**：列表滚动；Popover 不超过屏幕高。打开时总在顶部，不自动滚到崩溃的行；Crashed / Failed 行不置顶，顺序不变。崩溃的提醒靠菜单栏图标红点和 `Notify on crash` 通知。
- **长命令 / monorepo**：`pnpm --filter storybook-docs-preview dev` 这类命令在 238 pt 文字列下必然截断，tooltip 必须有。同一 Project 下相同命令在不同子目录时，靠第二行的完整路径区分。
- **国际化**：英文和简体中文（需求 §4.1），中文规则见 §6.6。Rust 不产出界面文字，错误以枚举 + 参数传到 Swift 再本地化。第二行的状态词和按钮是最容易超长的位置。帮助文字截断可接受，但按钮文案（`Stop 2 Services and Quit`）不可截断——允许按钮变宽，alert 按钮纵向堆叠本来就容纳得下。所有带数量的文案用 String Catalog 复数规则，不要字符串拼接。
- **高频日志**：日志区只追加，不全量刷新；用户向上滚动离开底部时暂停自动滚动，回到底部恢复（隐式自动滚动开关，C4）。丢弃行时在底栏计数后显示 ` · 1,204 dropped`。
- **崩溃循环**：60 s 内崩 3 次后暂停自动重启，日志写系统行 `Crashed 3 times in 60 s. Auto restart paused.`，行显示 Crashed，按钮 Start。
- **Project 目录被移动 / 删除**（需求 §8.1-16）〔设计未画〕：第二行 `{path} · Folder missing`（`status-error-text`），Start 时 warning 条 `~/code/api is missing.` + `Locate…`。标题行消失后，原来的警告图标需要这个新位置。
- **node / bun 不存在**（需求 §8.1-15）：Run 进入 Failed，日志系统行写明找不到哪个可执行文件，Advanced 的对应行帮助文字变红 `Not found`。
- **外部进程在冲突条显示期间退出**：冲突条自动消失（现有逻辑每秒轮询）。
- **Supervisor 断连**（需求 §5.3）〔设计未画〕：Popover 顶部 warning 条 `Lost connection to the Supervisor.` + `Resync`；所有启停按钮 disabled。
- **Popover 打开时状态变化**：行原地更新，不重排顺序（顺序 = Project 登记顺序 + Service 创建顺序）。
- **Agent Server 出现或退出**：在所属 Project 的位置插入或移除这一行，其他行不动。分组模式下 Project 因此变空时，小标题一起消失。
- **`launch.json` 无法解析**：当作没有配置，不识别这个 Project 的 Agent Server，诊断日志记一行。不在界面报错（这个文件归 agent 管）。
- **同一配置被 agent 启动两次**：两行都显示，名字相同，靠 tooltip 里的 PID 区分〔设计未画〕。
- **任意可执行文件**（需求 §1.1）：行和 Service 视图与 Node/Bun 命令完全相同。这类命令没有常用脚本的预设，整条显示，也不会被判成一次性服务。

---

## 8. Animation / Motion

| 元素 | 触发 | 动画 | 时长 | 缓动 |
|---|---|---|---|---|
| 行 hover | 鼠标进入 | 背景色 | 100 ms | ease |
| 箭头 ↔ 启停按钮 | 悬停、键盘聚焦 | 交叉淡变 | 100 ms | ease |
| 色块 spinner | busy | 旋转 360° 循环 | 0.9 s | linear |
| 色块状态切换〔设计未画〕 | 状态变化 | 底色和图标淡变 | 150 ms | ease-out |
| 状态点 | busy 状态（Service 视图） | opacity 1 → .25 → 1 循环 | 1 s / 周期 | ease-in-out |
| Spinner | Reloading、Stopping all | 旋转 360° 循环 | 0.9 s | linear（用系统 `ProgressView().controlSize(.small)` 即可） |
| 开关 | 切换 | 底色 + 滑块位移 14 pt | 150 ms | ease |
| 视图切换 | 进入子视图 / Back | 横向推入（进入从右、Back 从左）；Popover 高度变化跟随系统 | 150 ms | ease-out |
| 内联确认 | 出现 | 插入并淡入 | 150 ms | ease-out |
| Toast | 出现 / 消失 | 淡入 / 淡出 | 150 ms / 200 ms | ease-out |
| 外部进程行 | SIGTERM 已发 | opacity → .45 | 即时 | — |

开启“减弱动态效果”（Reduce Motion）时：状态点不脉冲，视图切换无位移，改为淡变。

---

## 9. Accessibility

画布颜色对比计算（WCAG 2.1 AA，正文需 4.5:1，非文字 UI 需 3:1）：

| 组合 | 比值 | 结论 |
|---|---|---|
| `#6C6C70` 第二行 on Popover | 4.9 | 通过 |
| `#6C6C70` 第二行 on 行 hover | 4.4 | 略低于 4.5 → 用系统 `secondaryLabelColor` 后复测 |
| `#B42318` Crashed on Popover / hover | 6.1 / 5.5 | 通过 |
| 色块图标 Running / Busy / Crashed / Stopped | 4.4 / 4.8 / 5.4 / 4.2 | 通过（非文字 3:1） |
| `#48484A` 端口胶囊 | 7.4 | 通过 |
| `#0A63D6` `Add Service`、链接 on Popover | 5.2 | 通过 |
| 白字 on `#0A63D6` 主按钮 | 5.6 | 通过 |
| `#5C3A00` on `#FFF4E0` 冲突条 | 9.4 | 通过 |
| `#AEAEB2` 行尾箭头 | 2.1 | 低于 3:1。属装饰，整行可点的提示另有 hover 底 |
| `#C9C9CE` 开关关闭 on `#F5F5F7` | 1.5 | 用系统 Toggle 替代 |
| 日志各色 on `#1C1C1E` | 5.2–6.0 | 通过 |

要求：

- **状态不只靠颜色**：Running 和 Stopped 的色块图标相同，只差颜色，所以第二行的状态词不能去掉。VoiceOver label 包含状态。
- **Service 行**：画布是“一个大按钮 + 一个图标按钮”。落地为一个 accessibility 元素：label `"{displayName}, {状态}, port {N}"`，默认动作 = 打开设置，`accessibilityAction(named: "Start"/"Stop")` 暴露启停，避免读两遍。用 displayName 是因为 monorepo 的命令读出来难区分。
- **Agent Server 行**：一个 accessibility 元素，label `"{name}, Running, port {N}, started by Claude"`，没有默认动作，`accessibilityAction(named: "Stop")`。
- **分组小标题**：header trait，VoiceOver 用转子可以按 Project 跳。键盘 ↑ / ↓ 跳过小标题。
- **图标按钮都要 label + tooltip**：`Stop api › dev`、`Start api › dev`、`Back` / `Back to api › dev`、`Open :3000 in browser`。色块、箭头 `accessibilityHidden`。
- **开关**：label = 行标题，value = on/off；帮助文字作为 `accessibilityHint`。
- **日志区**：`accessibilityRole = .textArea`/log，label `Logs of api › dev`；新行**不**自动播报（会刷屏）。
- **Toast / 内联提示**：出现时用 `NSAccessibility.post(.announcementRequested)` 播报一次；冲突条、确认卡片出现时焦点移到其主按钮。
- **焦点顺序（Popover Home）**：`Add Service` → 每个 Service：行 → 行尾按钮 → Other processes → `Settings` → `Quit`。
- **打开时不显示焦点框**（画布注释），否则每次打开第一行都带一圈蓝框。键盘事件要落到列表上：列表容器获得焦点并用 `.focusEffectDisabled()` 隐藏外观，第一次按 ↓ 选中第一行（↑ 选中最后一行），现有的 `focus(after:in:step:)` 对 nil 已经这样处理。需要真机验证。
- **键盘**（设计未画，按需求 §4.1 补齐）：

| 键 | 位置 | 行为 |
|---|---|---|
| ↑ / ↓ | Home | 在 Service 行间移动 |
| Return | Home 行 | 打开 Service 视图。Agent Server 行无动作 |
| Space | Home 行 | 启动 / 停止，busy 时无效。Agent Server 行只能停止 |
| ⌘R | Home 行 | 运行中为 Restart |
| Esc | Popover | 有内联确认时先取消确认。否则看键盘焦点：有焦点时（输入框、Home 清单）先关警告条，再返回上一级，Home 上关闭 Popover；没有焦点时（如刚进 Service 视图）直接关闭 Popover（2026-11-09 定） |
| ⌘F | Logs | 聚焦搜索框 |
| ⌘K | Logs | Clear |
| ⌘, / ⌘Q | 任意 | Settings / Quit |

- 对话框：Return = 默认按钮，Esc = Cancel/Keep Running。**破坏性操作（Remove、Force Kill）的默认按钮是取消**，退出对话框的默认按钮是 `Stop N Services and Quit`（画布注释明确如此）。
- 焦点环：系统默认（画布模拟为 3 pt `accent` 35% 外描边）。Service 行除外，键盘选中和 Hover 一样只有整行底色。

---

## 10. 文案清单（给 String Catalog）

```
Popover
  "Bunode" · "%d running · %d Services" · "Add Service" · "Settings" · "Quit"
  "Crashed" · "Failed" · "Starting…" · "Running" · "Stopping…" · "Restarting…" · "Stopped" · "Not responding" · "Folder missing"
  "Run" · "Run %@" · "Running…" · "Done"
  "No commands yet." · "No Projects yet." · "Add Project…"
  "Other processes" · "%d started outside Bunode" · "None right now"
  "Run #%d" · "No Runs yet"
  "Port %d is used by “%@” from %@. Bunode won’t stop it." · "Port %d is used by another program. Bunode won’t stop it." · "Use :%d" · "Show Process" · "Cancel"
  "Sets PORT=%d for this Run. Tools that ignore PORT need their own flag."
  "Command" · "Folder" · "Change…" · "Port" · "Checked before each start" · "none"
  "Restart if it crashes" · "Pauses after 3 crashes in 60 s"
  "Run through shell" · "For pipes, && and globs"
  "Environment" · "%d variables" · "None · .env files are not read" · "Edit…"
  "Add Variable" · ".env files are not read. Your tools load them as usual." · "Value of %@" · "Invalid name" · "Already set"
  "Logs" · "%d lines" · "Saved. Applies on the next Run." · "Restart now"
  "Delete Service…" · "It is running and will be stopped first. " · "Only the Service is removed; files in %@ stay."
  "Stop and Delete" · "Delete" · "Deleted %@. Files in %@ are untouched."
  "New Service" · "Runs from %@" · "Project" · "Scripts in package.json" · "Add" · "Added %@" · "Name already used in %@"
  "Logs · %@" · "Search" · "No output yet." · "No lines match." · "%d lines · %d matches" · "Clear" · "Copy" · "Export…" · "%d dropped" · "Restart" · "Start" · "Stop"
  "Started outside Bunode · watch only" · "Stop…" · "Exiting…"
  "Send SIGTERM to PID %d? Bunode didn’t start it and can’t bring it back." · "Send SIGTERM" · "SIGTERM sent to PID %d"
  "No other Node or Bun processes."
  "Started by %@" · "Open in Browser" · "Show in Finder" · "Couldn’t stop %@."
  "%d Services are running. Quitting stops every Run." · "Stop %d Services and Quit" · "Stopping all Runs…"
  "%@ is missing." · "Locate…" · "Lost connection to the Supervisor." · "Resync"
Settings
  "General" · "Projects" · "Advanced" · "Source code"
  "Open at login" · "Bunode lives in the menu bar."
  "Show running count" · "Next to the menu bar icon."
  "Restart Services that crash" · "Up to 3 times a minute, then it waits for you."
  "Notify on crash" · "One notification per crash."
  "Remove…" · "Auto (%@)" · "Several lockfiles found. Pick a package manager." · "No Projects yet." · "Services are added and set up from the menu bar." · "Add Project…" · "Project name" · "Click to rename"
  "Remove “%@”?" · "Its %d Services leave Bunode. Files in %@ stay on disk." · " %d running Services will be stopped first."
  "Stop %d and Remove" · "Remove" · "Removed %@. Files untouched."
  "Shell environment" · "%d variables from %@ · %@" · "Reload" · "Reloading" · "Environment reloaded. New Runs get it."
  "Node.js" · "Bun" · "Force stop after" · "Stop asks nicely first, then force-quits."
  "Log lines kept" · "Per Service. Export saves the full log." · "Diagnostics" · "Supervisor and runner logs." · "Open"
  "List" · "How the menu bar list is arranged." · "Flat" · "By Project"
  "Language" · "Applies after Bunode restarts." · "System" · "English" · "简体中文" · "Restart Bunode to apply." · "Restart Now"
  "Show dev servers started by agents" · "Servers an agent such as Claude starts from its launch settings in a Project's folder. You can stop them here."
Dialogs
  "Quit Bunode?" · "%d Services are running. Bunode stops every Run before it quits, so it never leaves processes behind."
  "Stop %d Services and Quit"
  "Force kill %@ (PID %d)?" · "Bunode didn’t start this process. It did not exit %d s after SIGTERM. Force killing it may lose unsaved work."
  "Force Kill" · "Keep Running"
  "Restart Bunode?" · "%d Services are running. Bunode stops every Run before it restarts." · "Stop %d Services and Restart"
```

所有 `%d` 文案走复数规则（`1 Service` / `2 Services`）。`English` 和 `简体中文` 标为不翻译。Rust 错误枚举的文案在实现时逐个补进 String Catalog。中文译文见 §6.6。

---

## 11. 待定事项汇总

下面几处新设计和已定决策或现有行为冲突，需要产品决定。“默认”是不做决定时本文采用的做法。

| 问题 | 冲突 | 默认 |
|---|---|---|
| 停止时要不要显示端口 | 画布只在运行中显示。提交 `3e60f34` 刚恢复了行上的端口，停止时显示灰色配置端口 | 只在运行时显示，配置端口在 Service 视图能看到 |
| Service 视图开关文案（C2） | 画布写 `Use login shell`，C2 和代码是 `Run through shell`（`For pipes, && and globs`） | 保留 `Run through shell`，画布是旧文案 |
| Settings 里能否改包管理器（C6） | 现有 `ProjectsPane` 有 Picker 和多锁文件警告，极简版 Settings 只显示文字 | 仅在检测到多个锁文件时显示 Picker |
| 条目的第二行 | 画布只显示 Project 名。`pnpm dev` 分布在两个子目录时无法区分 | 显示运行目录的路径，Project 名移到第一行脚本名前（2026-11-09 定） |
| Popover 背景 | 曾定为接近不透明，Llama 参考图是系统半透明材质 | 系统默认材质（2026-11-09 定） |
| 命令显示 | 画布写完整命令，包管理器前缀灰色 | 常用脚本只写脚本名，其余整条（2026-11-09 定） |
| 短时 Service | 画布只画常驻服务，构建跑完会显示成 `Stopped` | `build` `test` `lint` 单独显示，不计入 running 数量（§5.1，2026-11-09 定） |

此前已定（2026-11-06）：Settings 窗口固定 700×540 不可拉伸；`Open at login` 默认关；环境变量用 Popover 内的 Environment 视图；Crashed / Failed 行不自动滚动（“只置灰”已被 C7 的本版状态取代）。
