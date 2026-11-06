# 实机验证清单

自动化测试已经在 CI（macOS runner）上覆盖了 core、runner 和 Swift ViewModel，但界面本身（菜单栏、Popover、Settings 窗口、对话框、键盘与 VoiceOver）只有在真机上才能确认。按下面的顺序逐项勾选即可；括号里是对应的 issue。

## 0. 准备

- [ ] macOS 26、Xcode 26、rustup、XcodeGen（`brew install xcodegen`）已安装；node、npm、pnpm、bun 在终端里可用
- [ ] 拉取 `main` 后在仓库根目录运行 `make`，再运行 `make run`
- [ ] 下文的 `fixtures/…` 都指仓库里的同名文件夹，可以直接登记为 Project

## 1. 菜单栏与启动（#1 #4 #5）

- [ ] 没有 Dock 图标，也没有 App 菜单栏；菜单栏出现六边形图标
- [ ] 点击图标打开 Popover（宽 340 pt），再点一次关闭，不会关了又立刻弹开
- [ ] 每次重新打开 Popover 都回到 Home
- [ ] 中心点：没有运行时为灰色；有 Run 时为绿色；有 Crashed 或 Failed 时为红色
- [ ] 图标右侧显示运行数，为 0 时不显示；关闭 Settings › General › Show running count 后也不显示
- [ ] 开启 VoiceOver（⌘F5），图标读作 `Bunode, 1 running`，有崩溃时读作 `Bunode, 1 running, 1 crashed`

## 2. Settings 与 Project（#2 #13）

- [ ] ⌘, 打开 Settings：700×540、不能调整大小；侧栏有 General、Projects、Advanced 和 Source code 链接
- [ ] `Add Project…` 选 `fixtures/monorepo-pnpm`，Toast 显示 `Added … (pnpm). Add its Services from the menu bar.`；再添加一次同一目录，显示 `… is already added`
- [ ] 新建一个文件夹，里面放 `yarn.lock` 和 `package-lock.json`，登记后行上有橙色警告和 tooltip；在包管理器菜单里选 npm，警告消失
- [ ] 退出后重新打开 App，Project 列表不变
- [ ] 把 `~/Library/Application Support/Bunode/config.json` 改成乱码后启动：不崩溃，Projects 页显示无法读取的提示，同目录下留有 `config.unreadable-*.json`
- [ ] `Remove…`：没有 Run 时按钮为 `Remove`，有 Run 时为 `Stop 1 and Remove`；默认按钮是 Cancel（直接回车即取消）；确认后显示 Toast，Finder 里项目文件仍在
- [ ] 在 Finder 里把一个已登记的项目文件夹改名：Home 和 Settings 出现警告；点该 Project 下某个 Service 的 Start，出现 `~/… is missing.` 和 `Locate…`；选中改名后的文件夹即恢复

## 3. Service 的添加、编辑与删除（#3 #11 #12）

- [ ] 点 Project 小标题的 `+` 打开 New Service：显示脚本胶囊，默认选中第一个未被使用的脚本；pnpm 项目生成 `pnpm dev`、npm 项目生成 `npm run dev`
- [ ] 手动修改 Command 后，胶囊取消选中；名字重复时 Add 不可用，并显示 `Name already used in …`
- [ ] Add 后回到 Home，Toast 显示 `Added … › …`；重启 App 后 Service 仍在
- [ ] 对没有 `package.json` 的文件夹点 `+`：不显示脚本卡片，焦点在 Command
- [ ] Monorepo：登记 `fixtures/monorepo-npm`，New Service 里的胶囊按子包分组；添加 `apps/web` 的 `where`，Home 显示 `web › where`；启动后日志里打印的 cwd 以 `apps/web` 结尾
- [ ] Service 视图里修改 Command、Port 后，失焦或回车即保存；Port 只能输入数字；Folder › `Change…` 选到 Project 以外的目录时被拒绝
- [ ] Service 运行中修改设置：出现 `Saved. Applies on the next Run.` 和 `Restart now`；点 `Restart now` 后提示消失，日志第一行是新命令
- [ ] Run through shell：命令填 `echo a && echo b | tr a-z A-Z`；关闭开关时输出一整行原样参数，打开后输出 `a` 和 `B`
- [ ] `Delete Service…`：运行中显示 `It is running and will be stopped first…` 和 `Stop and Delete`；完成后回到 Home，Toast 说明文件未动

## 4. 启停与状态（#4 #5 #10 #14）

- [ ] `fixtures/node-http` 里添加 Service `node server.js`：Start 后先显示 spinner，再变绿点；Stop 后变灰；在终端执行 `pgrep -fl server.js` 确认进程已不在
- [ ] `fixtures/process-tree` 里添加 `npm run tree -- /tmp/bunode-pids`：Stop 后执行 `xargs ps -p < /tmp/bunode-pids`，三个进程都不在
- [ ] 命令 `nosuchcmd` → Failed，备注 tooltip 写明 `nosuchcmd wasn't found in PATH.`；`fixtures/crash` 的 `node immediately.js` → Crashed，菜单栏中心点变红；行的顺序不变
- [ ] 添加 `node -e "process.on('SIGTERM',()=>{});setInterval(()=>{},1000)"`：Stop 后约 5 s（Advanced › Force stop after）被强制结束；改成 30 s 再 Stop，约 35 s 后行显示 `Not responding`，再点 Stop 会强杀
- [ ] Restart（Logs 顶栏、右键菜单或选中行后按 ⌘R）：中间显示 Restarting，结束后 `Run #` 加 1；连按多次也只有一个进程
- [ ] 快速连点同一行的 Start/Stop 十几次，最终最多只有一个进程
- [ ] 有 Run 时点 Popover 里的 Quit Bunode：出现内联确认，然后显示 `Stopping all Runs…`，退出后所有 Run 的进程都不在
- [ ] Settings 窗口在前台时按 ⌘Q：弹出 `Quit Bunode?` 对话框，回车即 `Stop N Services and Quit`
- [ ] 强杀 App：`kill -9 $(pgrep -x Bunode)`，几秒内所有 Run 的进程都会结束（runner 的生命线）
- [ ] 只杀 runner：Run 运行时执行 `pkill -9 bunode-runner`，该行变为 Crashed，命令的进程也随之结束（Bunode 自己结束了进程组）
- [ ] 遗留清理（可选）：Bunode 运行着 `fixtures/process-tree` 时，用一条命令同时杀掉两者：`kill -9 $(pgrep -x Bunode) $(pgrep bunode-runner)`，node 进程会残留；重新打开 Bunode，几秒内残留被结束，`~/Library/Logs/Bunode/supervisor.log` 里有 `Ended the leftover process group`
- [ ] Advanced › Diagnostics › `Open` 在 Finder 中打开 `~/Library/Logs/Bunode`，里面有 `supervisor.log` 和 `runner.log`

## 5. 运行环境（#6）

- [ ] 退出后从 Finder 双击 `Bunode.app` 启动（不经过终端）：用 nvm 或 fnm 安装的 node 的项目也能正常启动
- [ ] Advanced › Shell environment 显示变量数量、shell 和时间；`Reload` 期间显示 spinner，结束后弹出 Toast
- [ ] Node.js 和 Bun 两行显示路径和版本；`Change…` 换成另一个 node 后，新 Run 使用它（可用 `node -v` 作为 Service 命令验证）；`Use PATH` 可以恢复

## 6. 日志（#7 #8 #9）

- [ ] Logs 视图：每行有时间戳，保留 ANSI 颜色（例如 Vite 的输出），stderr 为红色，Bunode 的系统行为灰色；浅色模式下日志区仍为深色
- [ ] `fixtures/flood` 的 `node flood.js`：Popover 不卡，能滚动和搜索；底栏显示 `10,000 lines · … dropped`
- [ ] 向上滚动后不再跟随新行，滚回底部后恢复跟随
- [ ] `Clear` 清空显示；`Copy` 复制全部；选中若干行后按 ⌘C 只复制选中的行
- [ ] 搜索：⌘F 聚焦搜索框，输入后底栏显示 `N lines · M matches`，无结果时显示 `No lines match.`；从 Service 视图重新进入 Logs 时搜索词被清空
- [ ] `Export…` 默认文件名为 `<project>-<service>-run<N>.log`，内容是完整日志，包括超出 Log lines kept 而被丢弃的行
- [ ] Advanced › Log lines kept 有 5,000、10,000、50,000 三档，对之后的新 Run 生效

## 7. 端口（#16 #17）

- [ ] `fixtures/node-http` 启动后，行上以蓝色显示实际端口；点 Service 视图里的端口链接，浏览器打开后显示 `ok`
- [ ] `fixtures/multi-port` 启动后显示 `:xxxx +1`
- [ ] 端口冲突：先在终端执行 `cd fixtures/node-http && PORT=4321 node server.js`，再把某个 Service 的 Port 设为 4321 并 Start：行下出现冲突条 `Port 4321 is used by “node server.js” from Terminal. Bunode won’t stop it.`
- [ ] 点 `Use :4322`：本次 Run 的日志里有系统行 `PORT=4322 for this Run`，Service 的 Port 仍为 4321；`Show Process` 跳到 Other processes 并定位到该进程
- [ ] 在终端里 Ctrl-C 结束那个 node 后，冲突条自动消失

## 8. 其他进程与 External 行（#15 #18）

- [ ] 在终端执行 `cd fixtures/process-tree && npm run tree -- /tmp/p`：Header 芯片的数字加 1；Other processes 中显示 `npm run tree -- /tmp/p`、来源（Terminal、iTerm 或 VS Code 等）和 PID；npm 和它启动的 node 合并为一项
- [ ] `Stop…` → `Send SIGTERM`：行变淡并显示 `Exiting…`，进程退出后从列表消失
- [ ] 对忽略 SIGTERM 的外部进程（用第 4 节那条 node 命令在终端启动）：5 s 后弹出 Force kill 对话框，默认按钮是 `Keep Running`；选 `Force Kill` 后进程消失
- [ ] 把 `fixtures/process-tree` 登记为 Project 后，在终端里从该目录启动的进程会以 `External · …` 行显示在 Home 的这个 Project 下；点击后跳到 Other processes
- [ ] Bunode 自己启动的 Run 从不出现在 Other processes 里

## 9. 键盘、VoiceOver 与外观（#19）

- [ ] 打开 Popover 后焦点在第一个 Service 行（若看不到焦点环，在系统设置 › 键盘里打开“键盘导航”）；↑/↓ 在行间移动，Return 打开 Service 视图，Space 启停，⌘R 重启
- [ ] Esc：先关闭内联确认或提示条，再逐级返回，在 Home 时关闭 Popover
- [ ] VoiceOver：Service 行读作 `acme › dev, Running, port 3000`，转子的“操作”里有 Start 或 Stop；Toast 和冲突条会播报，日志新行不会
- [ ] 开启系统设置 › 辅助功能 › 显示 › 减弱动态效果后，状态点不再闪烁，视图切换不再滑动
- [ ] 切换深色和浅色模式，Popover 与 Settings 都跟着变化

## 10. 稳定性与性能（#20，可选）

- [ ] 在 `fixtures/node-http` 下添加 30 个 Service（例如 `node server.js --n1` 到 `--n30`），全部启动后 Popover 滚动流畅
- [ ] 空闲时用 `top -pid $(pgrep -x Bunode) -l 10 -stats cpu,mem` 查看，CPU 应接近 0%，内存约在 50 MB 左右
- [ ] 运行 `make soak` 做长时测试（默认 30 分钟日志加 24 小时启停，可用 `BUNODE_SOAK_SECS` 缩短）；其余指标的测法见 `docs/stability.md`
