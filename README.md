# LogLume（LumeLog）

面向嵌入式 / HIL 测试工程师的轻量日志编辑器，也可作为日常文本编辑器使用。保留 Notepad++ 熟悉的操作习惯与宏能力，同时原生内置超大日志、工控编码、DBC/CAN 报文解析能力。

- 仓库：<https://github.com/HackerSnail/LogLume> · <https://gitee.com/hackerSnail/log-lume>
- 应用标识：`com.lumelog.desktop`，窗口标题 `LogLume`
- 许可证：仓库尚未声明，使用前请留意

## 主要能力

- **超大日志**：分块解码 + LRU + 后台流式索引，按需取窗的虚拟列表；大于 200MB 自动进入只读模式，不会一次性把文件读进内存。
- **多编码**：UTF-8 / UTF-16、GBK / GB18030 / Big5 / Shift-JIS、EUC-JP/KR、KOI8-R/U 等自动识别与切换，编码/行尾在保存导出时保持一致。
- **CAN / 工控数据**：DBC 导入与信号解码、candump/ASC/正则/CAN FD/BLF 报文源、MF4（MDF4）测量数据表格与波形、报文过滤及 CSV/JSONL 导出。
- **检索与导出**：行过滤与高亮、时序筛选、目录级「在文件中查找」、替换预览、筛选片段导出、CSV/TSV 分隔表格视图。
- **编辑体验**：CodeMirror 6 语法高亮（数十种语言）、正则查找替换、行操作/格式化、书签、宏录制与快捷键、命令面板（`Ctrl/⌘+Shift+P`）、可自定义快捷键。
- **工程化**：Git 状态/提交/分支/远端/冲突面板、QuickJS 逐行脚本、WASM 解析插件、xlsx/docx/pdf 查看、崩溃恢复、外部修改监听、会话恢复、文件关联（`.log`）与单实例。
- **AI 助手（可选）**：默认关闭，需在设置中显式启用并填写自有接口与 Key 才会联网；Key 明文保存在本机 `config.toml`，不随仓库分发。
- **界面**：浅色 / 暗黑 / Dracula 主题（可跟随系统）、三栏布局、文档信息栏；macOS 使用系统原生菜单栏与中文右键菜单。

完整交付清单见 [docs/features.md](docs/features.md)，设计文档见 [docs/specs](docs/specs)，测试口径见 [docs/test-checklist.md](docs/test-checklist.md)。

## 技术栈

| 层 | 技术 |
| --- | --- |
| 文档/解析内核 | Rust（workspace，edition 2021，MSRV 1.89） |
| 桌面外壳 | Tauri v2（`tauri-plugin-dialog` / `single-instance`） |
| 前端 | Vite 6 + TypeScript 5 + CodeMirror 6 + TanStack table-core |
| 测试 | Vitest 2 + `cargo test` |

## 目录结构

```
Cargo.toml                 # workspace: lumelog-core / lumelog-can / lumelog-app
crates/lumelog-core/       # 纯 Rust 文档引擎：分块解码、行索引、过滤、导出
crates/lumelog-can/        # DBC / CAN 报文解析与 golden 对拍数据
app/src-tauri/             # Tauri IPC 命令、Git/MF4/脚本/插件、macOS 原生菜单
  tauri.conf.json          # 主窗口与打包配置
  tauri.macos.conf.json    # macOS 覆盖：原生装饰 + Overlay 标题栏（红黄绿按钮）
app/frontend/              # Vite + TypeScript 界面（src/main.ts 为入口）
docs/                      # 功能清单、规格、计划、评审与测试清单
CHANGELOG.md               # 每次提交的改动与验证记录
```

## 环境要求

- Rust 1.89+（`rustc`、`cargo`），Node.js 20+（开发环境实测 Node 24 / npm 11）
- Tauri v2 的平台前置依赖：macOS 需 Xcode Command Line Tools；Windows 需 WebView2；Linux 需 WebKit2GTK 等系统包（详见 Tauri 官方前置条件）

## 安装与运行

```bash
git clone https://gitee.com/hackerSnail/log-lume.git   # 或 GitHub 仓库
cd log-lume

# 1. 安装前端依赖
cd app/frontend && npm ci && cd ../..

# 2. 从仓库根目录启动（frontend 与 src-tauri 是同级目录，
#    直接在 app/frontend 里执行 npm run tauri dev 会找不到 tauri.conf.json）
ROOT="$PWD"
./app/frontend/node_modules/.bin/tauri dev \
  --config "{\"build\":{\"beforeDevCommand\":\"npm --prefix $ROOT/app/frontend run dev\"}}"
```

开发服务器为 `http://localhost:1420`。首次启动需要编译 Rust 依赖，耗时数分钟；之后增量编译很快。

只看界面、不需要 Tauri IPC 时，可在 `app/frontend` 下执行 `npm run dev` 用浏览器打开，此时文件选择、Git 等原生能力会提示不可用。

### 构建与测试

```bash
cd app/frontend
npm run build      # tsc --noEmit && vite build
npm test           # vitest run

cd ../..
cargo test --workspace
```

桌面安装包使用 Tauri CLI 的 `tauri build` 从仓库根目录执行；`app/src-tauri/tauri.conf.json` 中配置了 dmg / nsis / msi / deb / AppImage 目标与 `.log` 文件关联。

## macOS 说明

- `app/src-tauri/src/menu.rs` 构建系统原生菜单栏，按「文件 / 编辑 / 查看 / 数据 / 设置 / 窗口」分类，全中文标签；菜单项通过 `menu://action` 事件转发给前端已有动作，快捷键与内置键位一致。
- `app/src-tauri/tauri.macos.conf.json` 让窗口使用原生装饰与 Overlay 标题栏，因此 macOS 不显示自绘的 Windows 风格窗口按钮；Windows / Linux 仍使用无边框自绘标题栏。
- `app/frontend/src/contextmenu.ts` 提供应用内中文右键菜单；macOS 自动注入的英文 AutoFill / Dictation / Emoji 项在启动时被移除。
- `.gitignore` 忽略 `.DS_Store`、`._*`、`__MACOSX/`、`npm-out.txt` 等系统垃圾与临时文件；`crates/**/tests/data/*.log` 是测试夹具，仍纳入版本管理。

## 配置

用户配置位于应用配置目录的 `config.toml`，包含 `[keymap]` 快捷键覆盖、`recovery_enabled` 崩溃恢复开关以及可选的 `[ai]` 段（启用状态、接口地址、模型、Key）。请勿把含 Key 的配置文件提交到仓库。

## 贡献

1. 提交前在根目录 `CHANGELOG.md` **最上方**追加条目，写清改了哪些文件、为什么、验证命令与结果（项目约定，见 [docs/claude.md](docs/claude.md)）。
2. 保证 `npx tsc --noEmit`、`npm test`、`cargo test --workspace` 通过；涉及界面交互时补充人工冒烟记录。
3. 同时推送到 GitHub 与 Gitee，避免镜像落后：

```bash
git push origin main     # GitHub
git push gitee  main     # Gitee
```
