<h1 align="center">LogLume · LumeLog</h1>

<p align="center">
给「嫌 VS Code 太重、嫌 Notepad++ 打不开几 GB 日志」的人做的轻量编辑器。日常改文本该有的操作习惯、宏、语法高亮和 Git 面板都在，超大日志、工控编码、DBC/CAN 报文解析也在原生功能里。文件都在本地，不联网也能用。
</p>

仓库：[GitHub](https://github.com/HackerSnail/LogLume)。应用标识 `com.lumelog.desktop`，窗口标题 `LogLume`。仓库尚未声明许可证，商用或分发前请留意。

## 简介

VS Code 功能全，但启动和内存开销大，打开 GB 级日志容易把资源占满；Notepad++ 只在 Windows 上跑，大文件、GBK 日志、CAN 报文都得靠插件；Sublime Text 是闭源商业软件，没有原生的 DBC / MF4 解析；Notepad-- 同属轻量替代，大文件性能和工控场景覆盖还有差距。LogLume 既是处理台架、HIL 测试和嵌入式调试日志的工具：分块解码的大文件视图、GBK / GB2312 等老编码、DBC 信号解码与 MF4 测量数据；也是日常文本编辑器：多编码切换、正则查找替换、宏、语法高亮、Git 面板。

**适合谁用**：嵌入式、商用车动力电控、HIL 自动化测试的工程师用它打开台架产出的几 GB 连续日志、按 GBK / GB2312 读设备输出、看 DBC 解出的 CAN 信号；普通开发者用它改代码和配置文件，多编码、正则替换、宏、语法高亮和 Git 面板都在。要插件生态或 IDE 级开发，用 VS Code 更合适。

功能清单见 [docs/features.md](docs/features.md)，设计文档在 [docs/specs](docs/specs)，测试口径见 [docs/test-checklist.md](docs/test-checklist.md)，项目约定见 [docs/claude.md](docs/claude.md)。

## 功能亮点

* **超大日志**：分块解码 + LRU 缓存 + 后台流式行索引，虚拟列表只取当前窗口；大于 200MB 自动进入只读，不会把整个文件读进内存。CHANGELOG 里记过一次 CLI 基准：971,234,620 字节 / 16,777,217 行合成日志，release 构建含行索引约 590 ms。UI 首屏走流式索引，和这个 CLI 口径不是一回事。
* **工控编码**：UTF-8 / UTF-16、GBK / GB18030 / Big5 / Shift-JIS、EUC-JP / KR、KOI8-R / U 自动识别与切换，编码和行尾在保存、导出时保持一致。设备与 ECU 导出的老编码日志不用先转码。
* **CAN 与测量数据**：DBC 导入与信号解码，candump / ASC / 正则 / CAN FD / BLF 报文源，MF4（MDF4）测量数据的表格与波形视图，报文过滤及 CSV / JSONL 导出。
* **检索与导出**：行过滤与高亮、时序筛选、目录级「在文件中查找」、替换预览、筛选片段导出、CSV / TSV 分隔表格视图。
* **编辑体验**：CodeMirror 6 语法高亮（数十种语言）、正则查找替换、行操作与格式化、书签、宏录制与自定义快捷键、命令面板（`Ctrl+Shift+P`）。
* **Git 与扩展能力**：Git 状态 / 提交 / 分支 / 远端 / 冲突面板、QuickJS 逐行脚本、WASM 解析插件。
* **文档查看与恢复**：xlsx / docx / pdf 查看、崩溃恢复、外部修改监听、会话恢复、`.log` 文件关联与单实例启动。
* **AI 助手（可选）**：默认关闭，要在设置里显式启用并填写自有接口与 Key 才会联网。Key 明文保存在本机 `config.toml`，不随仓库分发。
* **界面**：浅色 / 暗黑 / Dracula 主题（可跟随系统）、三栏布局、文档信息栏、中文右键菜单。macOS 使用系统原生菜单栏与 Overlay 标题栏，Windows / Linux 使用自绘标题栏。

## 快速开始

**安装**：打包目标配在 `app/src-tauri/tauri.conf.json`，覆盖 nsis / msi / dmg / deb / appimage。CHANGELOG 记录过一次 Windows 本地 `tauri build`，产出 NSIS 5.66 MiB、MSI 8.95 MiB；其它平台的发行包以仓库 Releases 页为准。

| 平台 | 产物 | 安装方式 |
| --- | --- | --- |
| Windows 10+ | NSIS `.exe` 安装包 / `.msi` | 双击安装 |
| macOS | `.dmg` | 拖入 Applications |
| Linux | `.deb` / `.AppImage` | `sudo apt install ./xxx.deb`；AppImage 加执行权限后运行 |

Windows 需要 10 及以上，这是 Tauri v2 与 WebView2 运行时的最低要求。Linux 需要装好 WebKit2GTK 等 Tauri v2 平台前置依赖；macOS 需要 Xcode Command Line Tools。

打开后用 `Ctrl+O` 选日志文件，`Ctrl+Shift+F` 做行过滤与高亮，`Ctrl+G` 跳到指定行，`Ctrl+Shift+P` 搜命令。导入 DBC 后在报文面板看信号解码结果，MF4 文件有独立的表格与波形视图。

**从源码运行**：需要 Rust 1.89+ 与 Node.js 20+（开发环境实测 Node 24 / npm 11）。

```bash
git clone https://github.com/HackerSnail/LogLume.git
cd LogLume
cd app/frontend && npm ci && cd ../..

# frontend 与 src-tauri 是同级目录，要从仓库根调 tauri；
# 直接在 app/frontend 里执行 tauri dev 会找不到 tauri.conf.json
ROOT="$PWD"
./app/frontend/node_modules/.bin/tauri dev \
  --config "{\"build\":{\"beforeDevCommand\":\"npm --prefix $ROOT/app/frontend run dev\"}}"
```

开发服务器在 `http://localhost:1420`，首次编译 Rust 依赖要几分钟。只看界面、不需要 Tauri IPC 时，可以在 `app/frontend` 下执行 `npm run dev` 用浏览器打开，此时文件选择、Git 等原生能力会提示不可用。

**构建与测试**：

```bash
cd app/frontend
npm run build      # tsc --noEmit && vite build
npm test           # vitest run

cd ../..
cargo test --workspace
```

桌面安装包用 Tauri CLI 的 `tauri build` 从仓库根目录执行。

## 快捷键

macOS 上 `Ctrl` 对应 `⌘`（`app/frontend/src/keymap.ts` 里 Ctrl 同时匹配 Cmd）。默认只绑下面 6 个动作，关闭文档、侧栏、文档信息栏、报文面板、导入 DBC、解析报文、识别选区等动作默认未绑定，在「快捷键设置」里重绑；命令面板能搜到全部动作。

| 键位 | 作用 |
| --- | --- |
| `Ctrl+O` | 打开文件… |
| `Ctrl+S` | 保存 |
| `Ctrl+Shift+S` | 另存为… |
| `Ctrl+Shift+F` | 行过滤 / 高亮命中… |
| `Ctrl+G` | 跳转到行… |
| `Ctrl+Shift+P` | 命令面板 |

## 技术栈

| 层 | 技术 |
| --- | --- |
| 文档 / 解析内核 | Rust（workspace，edition 2021，MSRV 1.89） |
| 桌面外壳 | Tauri v2（`tauri-plugin-dialog` / `single-instance`） |
| 前端 | Vite 6 + TypeScript 5 + CodeMirror 6 + TanStack table-core |
| 测试 | Vitest 2 + `cargo test` |

## 目录结构

```
Cargo.toml                 # workspace: lumelog-core / lumelog-can / lumelog-app
crates/lumelog-core/       # 纯 Rust 文档引擎：分块解码、行索引、过滤、导出
crates/lumelog-can/        # DBC / CAN 报文解析与 golden 对拍数据
app/src-tauri/             # Tauri IPC 命令、Git / MF4 / 脚本 / 插件、macOS 原生菜单
  tauri.conf.json          # 主窗口与打包配置
  tauri.macos.conf.json    # macOS 覆盖：原生装饰 + Overlay 标题栏
app/frontend/              # Vite + TypeScript 界面（src/main.ts 为入口）
docs/                      # 功能清单、规格、计划、评审与测试清单
CHANGELOG.md               # 每次提交的改动与验证记录
```

## 配置

用户配置位于应用配置目录的 `config.toml`，包含 `[keymap]` 快捷键覆盖、`recovery_enabled` 崩溃恢复开关，以及可选的 `[ai]` 段（启用状态、接口地址、模型、Key）。不要把含 Key 的配置文件提交到仓库。

## 贡献指南

1. 提交前在根目录 `CHANGELOG.md` 最上方追加条目，按时间倒序排列，写清改了哪些文件、为什么、验证命令与结果。这是 [docs/claude.md](docs/claude.md) 里的项目约定。
2. 保证 `npx tsc --noEmit`、`npm test`、`cargo test --workspace` 通过；涉及界面交互时补人工冒烟记录。
3. GitHub 与 Gitee 两个仓库都要推送，避免镜像落后。

Issue 和 PR 都欢迎，功能建议也可以直接提。

## 许可证

仓库尚未声明许可证，使用前请留意。
