# LogLume 界面重做（参考 NoteLume）实施计划

> **给实施者：** 本文是可照做的施工图。按 §8 的区块顺序推进，每个区块改完跑一次 `npm test`，并保留 §9 的 class 名契约。

**Goal:** 把 LogLume 的外壳从「原生控件拼装 + 网格线」改成克制、精致、以内容为先的桌面工具观感；视觉基线与 NoteLume 对齐（三层色阶、幽灵控件、hairline 分隔、120ms 级动效），但保留 LogLume 的工业工具信息密度。

**参考:** `https://github.com/HackerSnail/NoteLume`（master，2026-11-09 拉取，`src/style.css` 143KB / `src/index.html`）。

**范围:** `app/frontend/src/styles.css`、`app/frontend/index.html`、`app/frontend/src/main.ts`（仅 DOM 结构与 class 增量）、少量 `tabs.ts / statusbar.ts / listview.ts / canpanel.ts` 的样式钩子。**不改** Rust 端、不改 IPC、不改 `ROW_HEIGHT` 数学。

**分支:** `feat/ui-notelume`（从 `main` = `28d5882` 切出）。切分支前先停掉 main 分支上的 dev 实例（PID 4144），改完在新分支重新拉起。

---

## 1. 参考实现读后结论（NoteLume 为什么看起来"高级"）

| 维度 | NoteLume 做法 | 出处 |
| --- | --- | --- |
| 主题结构 | 三套预设同构；`--bg-primary/--bg-secondary/--text-primary/...` 为**源变量**，`--bg/--bg-panel/--fg/--muted/...` 是别名。面板改色只写源变量，全站自动跟随 | `style.css:3-102` |
| 分层方式 | **靠底色差分，不画硬边框**：侧栏/检查器 `background: var(--bg-panel)` 且显式注释"去掉左边框" | `style.css:655-701` |
| 侧栏 | 280px（可变）、`padding:10px`；文件树行 `padding:2px 6px; border-radius:4px`；选中态 = 透明底 + 左侧 3px 主题色竖条 + 600 字重 | `style.css:834-881` |
| 检查器 | 分区 `margin-bottom:18px`；分区标题 12px、大写、`letter-spacing:.5px`、muted；可折叠 `<details>` 的 chevron 用 mask + `rotate(-90deg)`，`transition .12s` | `style.css:716-765` |
| 工具条 | 一条 34px 高的标题栏：应用名 + 菜单 + 幽灵按钮（`border:1px solid transparent; border-radius:6px; hover 出 --hover`）+ 拖拽填充 + 34×28 窗口按钮；`active` 才用实心 accent | `style.css:167-256` |
| 图标 | 全站唯一 SVG sprite（`<symbol id="i-*">`）+ `.nl-ico{width:1em;stroke-width:1.7}`，颜色继承 `currentColor`，三主题自动换色 | `index.html:12-88`、`style.css:257-272` |
| 字体 | UI 13px；编辑器 17px / 1.75 / 最大宽 860px（≥1400px 放宽到 1024）；UI 栈 Inter→HarmonyOS Sans→Noto Sans SC→系统（避开宋体） | `style.css:109-118,1927-1946` |
| 圆角 | 4（行/小块）· 6（按钮/输入/标签页）· 8（查找栏/建议浮层）· 10（模态/命令面板）· 12（浮动工具条）· 999（chip） | 全文件 |
| 阴影 | 模态 `0 12px 40px rgba(0,0,0,.28)`；浮层 `0 8px 28px .22`；查找栏 `0 4px 16px .16`；浮动高亮条**双层阴影 + 顶边 1px 内高光 + backdrop blur 20px** | `style.css:2052-2075,1425-1433` |
| 表格 | 表头 600 + 底边 2px；斑马纹 `color-mix(fg 3.5%)`；行悬停 `color-mix(accent 8%)`；无逐格竖分隔线 | `style.css:3297-3351` |
| 动效 | 只有 100–150ms 的 ease：折叠箭头旋转、hover 底色、`:active{transform:scale(.96)}`；没有位移/弹跳动画 | `style.css:759,916,2118-2125` |
| 控件基线 | 所有按钮/输入都有显式 `border-radius`、`border`、`:hover`、`cursor`；聚焦只改 `border-color:accent`，不画 2px 外框 | 全文件 |

一句总结：**NoteLume 的"精致"来自统一的最小替换单元（1 套 token、1 套图标、4 档圆角、3 档阴影、1 种 hover 语言），而不是某个炫技样式。**

---

## 2. 现状诊断：LogLume 具体丑在哪

### 2.1 硬伤（结构性）

| # | 症状 | 位置 | 为什么难看 |
| --- | --- | --- | --- |
| D1 | 全站**没有任何表单基线样式**：没有 `button / input / select / textarea` 的基础规则，也没有 `:focus-visible` | `styles.css` 全文（grep 0 命中） | WebView2 的原生按钮/下拉/输入框带着系统边框、内阴影、系统字体；和自定义面板混在一起，一眼就是"网页套壳" |
| D2 | 工具条是 **10 个等权重文字按钮**平铺，无图标、无分组、无主次 | `main.ts:297-306`、`styles.css:123-134` | 10 个同级按钮 = 没有信息层级；"打开文件"和"主题"视觉权重一样，用户找不到主操作 |
| D3 | 标题栏与工具条**没有视觉关系**：应用名 13px/600 直接贴第一个按钮，拖拽区不可见，窗口按钮 42×30 与 6px 内边距的条子高度不匹配 | `styles.css:123-177` | 标题栏像"一堆按钮的容器"，不像 macOS/VSCode 那种分层的 chrome |
| D4 | 边框泛滥：侧栏 `border-right` + 检查器 `border-left` + `panel-title` `border-bottom` + **每一条列表项又 `border-top`** | `styles.css:195-222,228-256,804-817` | 标题下沿和第一行上沿并存 → 双线；逐行 hairline 把侧栏切成表格 |
| D5 | 底色几乎无差：浅色 `--bg #fff` vs `--bg-subtle #fbfcfd`（ΔL ≈ 1.6%） | `styles.css:13-31` | 侧栏、标签条、编辑区糊成一片白，只能靠边框分区 → 反过来强化 D4 |
| D6 | 零动效：全文件**只有 1 条 `transition`**（toast 的 opacity .2s） | `styles.css:1310` | 所有 hover/展开/弹层都是硬切换，手感"廉价" |
| D7 | 单档阴影 `--shadow`（浅色 .12 / 暗色 .50），所有浮层共用 | `styles.css:29,47,65` | 暗色下弹窗一圈厚重的黑晕；浅色下弹窗和背景贴死 |
| D8 | 圆角随机：2/4/6/8/10/12 六档混用，同一类控件不一致（`.insp-git-init` 4px、`.status-follow` 4px、`.palette-box` 8px、`.modal-3way-box` 10px） | 全文件 | 圆角不一致最容易被眼睛识别为"粗糙" |
| D9 | 字号档位随机：11/12/13/15/18，且 `.panel-title` 13px/600、`.side-option` 12px、`.side-item` 继承 13px | `styles.css:121-256,769-790` | 侧栏内部三种字号在同一屏出现，节奏乱 |
| D10 | 原生 `<select>` 直接进了状态栏和查询面板（编码/行尾/语法/时间格式/条件字段） | `statusbar.ts:50-55,200-205`、`main.ts:3876-3887` | 下拉箭头、边框、高度、字体全部不受主题控制 |
| D11 | 标签页没有关闭按钮，也没有 active 态与编辑区的连续感（active tab 圆角 4px + 四条边） | `tabs.ts:57-74`、`styles.css:1000-1020` | 只能中键关闭；标签像浏览器遗留物而非工作区页签 |
| D12 | 虚拟列表行高 20px、正文无左内边距、gutter 右对齐 `padding-right:10px` | `styles.css:1069-1094` | 13px 字 + 20px 行 → 行距 1.5 却几乎贴行；正文紧贴行号列 |

### 2.2 局部问题

- **侧栏空态** `.insp-empty` 只是 12px 灰字，无留白、无图标、无下一步动作（`styles.css:831-833`）。
- **检查器** 每行 key/value 都带 `border-bottom`（`styles.css:810-817`），像参数表；长路径 `word-break:break-all` 会断成两行。
- **查询面板** 头行直接塞 3 个 checkbox + 5 个按钮 + 输入框，无分组、无 28px 控件高度（`styles.css:1096-1134`、`main.ts:3836-3907`）。
- **CAN 表** 表头 20px、行 20px、每行 `border-bottom`，四列靠 flex 均分（`styles.css:1599-1639`），扫描性差。
- **Toast** 反色黑条 `background:var(--text)`，无阴影、无圆角层级（`styles.css:1296-1314`）。
- **状态栏** 4px 内边距 + gap 14 + 文字标签"编码/行尾/语法"，信息密度低但视觉噪点多（`styles.css:1281-1294`、`statusbar.ts:84-88`）。

---

## 3. 目标视觉方向

**一句话：内容（日志正文）是唯一主角，外壳退到背景里；分隔用"色阶 + 1px hairline"，不用网格线。**

四条原则：

1. **三层色阶**：`bg`（编辑区，最亮/最暗）→ `bg-panel`（侧栏/检查器/工具条/状态栏）→ `bg-elevated`（弹窗/浮层）。相邻层差 3–5% 明度，不加边框也能分区。
2. **幽灵控件**：按钮/下拉默认无边框、无底色；hover 出 `--bg-hover`，按下 `scale(.97)`；只有主操作（模态的主按钮、命令面板选中项）用 accent。
3. **一套控件尺寸**：工具条控件高 28、面板控件高 28、状态栏控件高 20、模态按钮高 30。所有行高是 4 的倍数。
4. **动效只表达状态变化**：90–160ms、`cubic-bezier(.2,0,0,1)`；只动 `background-color / color / opacity / transform(≤2px 或 1.04 倍) / rotate`。禁止大位移、弹跳、超过 200ms。

暗色主题关键词：**中性灰蓝、低对比边界、浮层靠阴影而不是描边**。吸血鬼（Dracula）主题保留配色身份，但沿用同一套结构与尺寸。

---

## 4. 设计令牌（Token）

> 落在 `styles.css` 顶部（替换现有 `:root[data-theme]` 三个块）。保留旧变量名做别名，避免一次性重写 35KB 规则。

### 4.1 颜色（源变量）

| 语义 | light | dark | vampire | 用途 |
| --- | --- | --- | --- | --- |
| `--bg` | `#ffffff` | `#1e2127` | `#282a36` | 编辑区/正文底 |
| `--bg-panel` | `#f5f5f7` | `#171a1f` | `#21222c` | 侧栏、检查器、工具条、状态栏、dock |
| `--bg-elevated` | `#ffffff` | `#23262e` | `#2d2f3d` | 弹窗、命令面板、浮层 |
| `--bg-input` | `#f0f0f2` | `#22252c` | `#313342` | 输入框/下拉/搜索框底色 |
| `--bg-hover` | `#ebebee` | `#262b33` | `#44475a` | 行/按钮 hover |
| `--bg-active` | `#e3e3e7` | `#2d333d` | `#4b4e63` | 按下/选中悬停 |
| `--text` | `#1d1d1f` | `#d8dee6` | `#f8f8f2` | 主文字 |
| `--text-2` | `#6e6e73` | `#9aa4b0` | `#bfc7d6` | 次级（key、状态、路径） |
| `--text-3` | `#8e8e93` | `#6f7681` | `#6272a4` | 三级（行号、占位、空态） |
| `--border` | `#dcdce0` | `#2b3038` | `#44475a` | 结构性 hairline |
| `--border-strong` | `#c8c8cd` | `#3a4048` | `#52556b` | 输入框/浮层边界 |
| `--accent` | `#0071e3` | `#409cff` | `#bd93f9` | 主操作/选中/焦点 |
| `--accent-soft` | `rgba(0,113,227,.10)` | `rgba(64,156,255,.14)` | `rgba(189,147,249,.16)` | 选中行底、标签 chip 底 |
| `--warn` | `#9a6700` | `#ffb340` | `#f1fa8c` | 外部改动/降级 |
| `--danger` | `#d70015` | `#ff453a` | `#ff5555` | 未保存/错误/关闭按钮 hover |
| `--success` | `#248a3d` | `#30d158` | `#50fa7b` | 保存成功等 |
| `--selection` | `rgba(0,113,227,.22)` | `rgba(64,156,255,.30)` | `rgba(189,147,249,.30)` | 文本选区 |

**兼容别名（必须保留，现有 35KB 规则直接引用）**

```css
--bg-subtle: var(--bg-panel);
--editor-bg: var(--bg);
--gutter-bg: var(--bg);
--muted: var(--text-2);
--faint: var(--text-3);
--border-soft: color-mix(in srgb, var(--border) 62%, transparent);
--shadow: var(--shadow-2); /* 旧规则仍引用 */
```

### 4.2 圆角

| Token | 值 | 用途 |
| --- | --- | --- |
| `--r-xs` | 4px | 行内小块、gutter 标记、进度条 |
| `--r-sm` | 6px | 按钮、输入、下拉、标签页、列表行 |
| `--r-md` | 8px | 查找栏、dock 卡片、建议浮层 |
| `--r-lg` | 10px | 模态、命令面板、侧栏分组卡片 |
| `--r-xl` | 12px | 浮动工具条（选区高亮条） |
| `--r-pill` | 999px | 标签 chip、计数徽标、toast |

规则：**同一控件族只允许一档**。行/按钮/输入=6；弹层=10（浮动小条=12）；chip=pill。

### 4.3 间距（4px 基）

`--sp-1:4` `--sp-2:8` `--sp-3:12` `--sp-4:16` `--sp-5:20` `--sp-6:24` `--sp-8:32`

配套尺寸：

| Token | 值 | 用途 |
| --- | --- | --- |
| `--h-toolbar` | 40px | 标题栏+工具条 |
| `--h-control` | 28px | 工具条/面板按钮、输入框、下拉 |
| `--h-control-sm` | 20px | 状态栏下拉、chip |
| `--h-tabbar` | 34px | 标签条 |
| `--h-row` | 30px | 侧栏/检查器行、命令面板行 |
| `--w-sidebar` | 280px | 侧栏默认宽（JS 下限 160/上限 480 不变） |
| `--w-inspector` | 288px | 检查器默认宽（JS 下限 200/上限 520 不变） |

### 4.4 字体

```css
--font-ui: "Inter", "HarmonyOS Sans", "Noto Sans SC", -apple-system, "Segoe UI",
  "Microsoft YaHei", Roboto, Helvetica, Arial, sans-serif;
--font-mono: "JetBrains Mono", "Fira Code", "SFMono-Regular", Consolas,
  "Liberation Mono", monospace;
--fs-11: 11px;  /* 三级元信息、分区大写标题 */
--fs-12: 12px;  /* 路径、计数、状态栏 */
--fs-13: 13px;  /* 控件与正文 UI（基准） */
--fs-15: 15px;  /* 命令面板输入、模态标题 */
--lh-ui: 1.5;
--lh-tight: 1.25;
--fs-editor: 12.5px;   /* 日志正文（密度优先） */
--lh-editor: 20px;     /* 必须等于 listview.ts 的 ROW_HEIGHT=20 */
--ls-caps: 0.04em;
```

**硬约束：** `ROW_HEIGHT = 20` 被 `listview.test.ts:28`（400px 视口 = 20 行）锁定，不允许改。正文只能通过字号/字重/内边距提可读性，**不能改行高**。

### 4.5 阴影与遮罩

| Token | light | dark / vampire |
| --- | --- | --- |
| `--shadow-1` | `0 1px 2px rgba(0,0,0,.06)` | `0 1px 2px rgba(0,0,0,.40)` |
| `--shadow-2` | `0 4px 14px rgba(0,0,0,.10), 0 1px 2px rgba(0,0,0,.06)` | `0 6px 20px rgba(0,0,0,.50)` |
| `--shadow-3` | `0 16px 44px rgba(0,0,0,.16), 0 2px 8px rgba(0,0,0,.08)` | `0 20px 52px rgba(0,0,0,.62)` |
| `--overlay` | `rgba(0,0,0,.24)` | `rgba(0,0,0,.45)` |

浮层统一：`background: var(--bg-elevated); border:1px solid var(--border); box-shadow: var(--shadow-3); border-radius: var(--r-lg);`，暗色下**不额外加深描边**。

### 4.6 动效

```css
--dur-1: 90ms;
--dur-2: 140ms;
--dur-3: 200ms;
--ease-out: cubic-bezier(.2, 0, 0, 1);
```

### 4.7 z-index 阶梯

`--z-resizer:4` `--z-sticky:6` `--z-find:30` `--z-toast:70` `--z-modal:80` `--z-drag:300`（替换散落的 10/20/60/65/66/70/80/85/300）。

---

## 5. 逐区域改造清单

### 5.1 工具条 / 标题栏（`main.ts:282-320` + `styles.css:123-177`）

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  LogLume   [⤒ 打开] [⌕ 查询] [▤ 报文] [◫ 测量] [▭ 查看] │ [ƒ 脚本] [⚙ 解析] │ …  ─ □ ✕ │ 40px
└─────────────────────────────────────────────────────────────────────────────┘
   ↑ brand 13/600     ↑ 主操作组（icon+12.5px 标签）      ↑ 工具组     ↑ 次级开关 icon-only
```

- 高度 40（`padding: 0 var(--sp-3)`），底边 1px `--border`，底色 `--bg-panel`。
- 左边：品牌字（13px/600，字距 `-0.01em`）+ 4px 圆点 accent 作为标识（纯 CSS `::before`，不新增 DOM）。
- 按钮：`height: var(--h-control); padding: 0 10px; border: 0; border-radius: var(--r-sm); background: transparent; color: var(--text-2)`；`hover { background: var(--bg-hover); color: var(--text) }`；`active { background: var(--bg-active); transform: scale(.97) }`。
- 分组：3 组之间插 `.appbar-sep`（1×20px，`--border-soft`），不再等距平铺。
- 图标：新增 sprite（见 5.2）+ `main.ts` 里 `mkBtn` 增加可选 icon 参数；**按钮仍保留文字标签**（可发现性优先），图标 14px、`gap:6px`。
- 右侧窗口按钮：46×32、`border-radius:0`（贴边），`hover` 用 `--bg-hover`，关闭键 hover `--danger`；图标改用 sprite（`i-win-min/max/close`），去掉 `﹣ ▢ ✕` 字符。
- 拖拽区 `titlebar-drag` 保持（Tauri 无边框必需），但不再影响布局高度。

**文件：** `styles.css`（重写 `.appbar` 段 123-177）、`main.ts`（`layout()` 内按钮分组 + `mkBtn` 加 icon 参数）、`index.html`（新增 sprite）。

### 5.2 图标系统（新增）

- 在 `index.html` `<body>` 顶部加 `<svg class="icon-sprite" aria-hidden="true">`，收录 14 个符号：file-plus、search、table、waveform、doc、script、plug、sidebar、info、sun/moon/contrast（主题循环）、win-min、win-max、win-close、chevron-down、close。
- `styles.css` 加 `.icon-sprite{display:none}`、`.ico{width:14px;height:14px;fill:none;stroke:currentColor;stroke-width:1.7;stroke-linecap:round;stroke-linejoin:round;flex:none}`。
- 主题按钮的图标随 `theme.current()` 切换 `href`（`main.ts` 里已有 theme cycle 回调，追加一行）。

### 5.3 侧栏（`main.ts:322-342,472-510` + `styles.css:195-256,769-790`）

```
┌────────────────────────────┐
│ 已打开文档            3 个 │  28px 区块头（12px caps, --text-2）
│                            │
│ │ main.log          修改中  │  30px 行；选中 = 3px accent 左条 + 600
│   app.log           只读   │
│   boot.txt                 │
│                            │
│ ─────────────────────────  │  hairline（唯一一条）
│ ☑ 崩溃恢复                 │  28px 脚注行
│ 快捷键设置…                │  幽灵行按钮
└────────────────────────────┘
```

- 宽度 `var(--w-sidebar)`，`background: var(--bg-panel)`，**删除右边框**（靠色阶分层），`padding: var(--sp-2) var(--sp-2) var(--sp-3)`。
- `.panel-title`：12px、`600`、`color: --text-2`、`letter-spacing: var(--ls-caps)`、大写英文/原中文、`padding: 6px 8px`、**去掉 border-bottom**；右侧加计数（`.panel-title .count`，11px `--text-3`）。
- `.side-list`：`padding: 2px 0`；`.side-item`：`height:30px; display:flex; align-items:center; gap:8px; padding: 0 8px; border-radius: var(--r-sm); border-left:0`；hover `--bg-hover`；active `background: var(--accent-soft); font-weight:600;` + `::before` 3px accent 竖条（`left:0; top:6px; bottom:6px; border-radius:0 3px 3px 0`）。
- `.side-meta` 改为 11px `--text-3`，不浮动，`margin-left:auto`。
- 脚注区：`.side-foot` 上边 1px `--border-soft`，`padding-top: var(--sp-2)`；`.side-option` 去掉逐行 border-top，行高 28；checkbox 用 `accent-color: var(--accent)`。
- 空态：`.insp-empty` → 居中块，`padding: 24px 12px`，11px/1.6 `--text-3`，顶部 24px 的 sprite 图标（`opacity:.6`）。

### 5.4 标签条（`styles.css:985-1040` + `tabs.ts:57-74`）

- `.tabbar`：高 34、`padding: 4px 8px 0`、`background: var(--bg-panel)`、底边 1px `--border`、`gap:2px`。
- `.tab`：`height:30px; padding: 0 10px; border-radius: var(--r-sm) var(--r-sm) 0 0; border:1px solid transparent; border-bottom:none; color: var(--text-2)`；hover `--bg-hover`；active `background: var(--bg); color: var(--text); border-color: var(--border)`，并在底部用 `box-shadow: 0 1px 0 var(--bg)` 盖住条子的 hairline（与编辑区连成一体）。
- 标签内顺序：`dirty-dot(6px 圆点) → tab-label → close(×)`。`dirty-dot` 为空时不占位（`width:0`），dirty 时 `width:6px; background: var(--accent); border-radius:50%`；`ext-dot` 保持文字 `●` 但改为 8px warn 色。
- 新增关闭按钮 `.tab-x`（16×16，hover `--bg-hover`，`opacity: .55 → 1`）。**行为**：调用现有 `cb.onClose`；不新增快捷键。
- 可选（P3）：中键关闭保留。

### 5.5 编辑器区（CodeMirror）与只读列表

```
┌──────────────────────────────────────────────────────┐
│  1 │ 2026-11-09 12:00:01.123  CAN 0x123 8 11 22 …     │  20px 行（锁定）
│  2 │ …                                                │
└──────────────────────────────────────────────────────┘
  ↑ gutter 右对齐 6ch，--text-3；正文左内边距 12px
```

- `.cm-editor`：`background: var(--bg)`；`.cm-gutters`：`background: var(--bg)`（**与正文同底**，去掉 `--gutter-bg` 色块）、`border-right: 1px solid var(--border-soft)`、`color: var(--text-3)`、`padding-right:6px`。
- `.cm-content`：`font-size: var(--fs-editor); line-height: var(--lh-editor); padding: 12px 0`；`.cm-line { padding: 0 16px 0 12px }`。
- 光标 2px accent；选区 `--selection`；`activeLine` `color-mix(accent 6%)`，`activeLineGutter` 只改文字色不铺底。
- `.listview`：`font-size: var(--fs-editor); line-height: var(--lh-editor)`；`.list-gutter`：`min-width:6ch; padding-right:12px; color: var(--text-3); font-variant-numeric: tabular-nums`；`.list-text { padding-right: 16px; padding-left: 12px }`；`.list-row:hover { background: color-mix(in srgb, var(--text) 4%, transparent) }`。
- **不动**：`ROW_HEIGHT`、spacer 高度计算、`visibleWindow`。
- Overview strip / bookmark gutter：颜色换 `--text-3`/`--accent-soft`，尺寸不变。

### 5.6 检查器（`main.ts:345-353,514-545,550-628,716-780` + `styles.css:804-897`）

```
┌──────────────────────────────┐
│ 文档信息                     │  28px 区块头
│ 路径        /home/a/main.log │  24px 双列基线；无逐行分割线
│ 编码        UTF-8            │  key 12px --text-2；value 12px mono
│ 行数        12,340           │
│ 大小        3.2 MB           │
│                              │
│ 标签                         │  11px caps 分区
│ (work)(can)(hil)             │  pill chip，accent-soft 底
│                              │
│ [初始化 Git]                 │  幽灵按钮 28px
└──────────────────────────────┘
```

- 宽度 `var(--w-inspector)`、`background: var(--bg-panel)`、**删除左边框**、`padding: var(--sp-2) var(--sp-3)`。
- `.insp-body`：`overflow:auto`、`padding-top: 2px`。
- `.insp-row`：`display:grid; grid-template-columns: 84px 1fr; gap:12px; align-items:baseline; padding: 2px 0; border-bottom: 0`；value 右对齐改左对齐（长路径不再两边飞），`word-break: break-word`（保留换行但不再截断字母）。
- `.insp-key`：11.5→12px、`--text-2`；`.insp-val`：12px、`font-variant-numeric: tabular-nums`；`.insp-val.warn` 保留 600。
- `.insp-section`：11px、大写、`--text-3`、`margin: 16px 0 6px`。
- `.insp-tags`：gap 6；`.insp-tag`：`border:0; background: var(--accent-soft); color: var(--accent); border-radius: var(--r-pill); padding: 2px 8px`；删除按钮 hover `--danger`。
- `.insp-git-init`：改成 `.btn` 基础类（见 5.11），28px 高、6px 圆角、hover `--bg-hover`。

### 5.7 查询 / 查找面板（`main.ts:3818-3960` + `styles.css:1096-1196`）

- `#search-panel`：`background: var(--bg-panel); border-top: 1px solid var(--border); max-height: 42%`。
- `.search-head`：`height:44px; padding: 8px 12px; gap:6px`；三个 checkbox 收进 `.query-opts` 组（`gap:12px`，11.5px `--text-2`），按钮组靠右。
- 输入：统一 `.field`（28px、`--bg-input`、1px `--border-strong`、6px 圆角、focus `border-color:var(--accent)` + `box-shadow: 0 0 0 3px var(--accent-soft)`）；**不用浏览器默认 focus 描边**。
- `.query-conds`：`padding: 0 12px 8px`；每条条件 `display:grid; grid-template-columns: 100px 1fr 100px 28px; gap:6px`。
- `.query-time`：独立一行，`padding: 0 12px 10px`；起止输入宽 12ch 不变。
- 面板出现：`opacity 0→1 + translateY(4px)`，`--dur-2`。

### 5.8 表格（CAN / MF4 / 解析器共用）

```
┌────────┬────────┬────────┬─────────────────────────────┐
│ 时间   │  ID    │ DLC    │ 数据                          │  28px 表头，600，底边 2px
├────────┼────────┼────────┼─────────────────────────────┤
│12:00:01│ 0x123  │ 8      │ 11 22 33 44 55 66 77 88       │  24px 行（见风险）
│12:00:02│ 0x456  │ 4      │ de ad be ef                   │
└────────┴────────┴────────┴─────────────────────────────┘
```

- 行高：`.can-row` 由 `ROW_HEIGHT` 驱动 = 20。**P0 不改**；P2 若放宽到 22，必须同改 `listview.ts` 与 `.can-hrow` 并全量跑测试（`visibleWindow` 的 400px/20 行断言会失败）→ 默认方案：保持 20，靠内边距与字号提观感。
- `.can-thead`：`background: var(--bg-panel)`，`.can-hrow` 600、`color: var(--text-2)`、底边 1px `--border-strong`（去掉每列的深色块）。
- `.can-row`：去 `border-bottom`；`hover` `color-mix(accent 7%)`；`position:absolute` 与 transform 逻辑不变。
- `.can-cell`：`padding: 0 10px`；数字列（时间/DLC）`font-variant-numeric: tabular-nums`；最后一列 `flex:2`。
- 表头与正文列宽必须继续用同一 flex 权重 → **只改 padding/字号，不改 flex 比例**。
- `.can-signals`：`background: var(--bg-elevated); border:1px solid var(--border); border-radius: var(--r-md)`。
- `.mf4-table-el`：thead `sticky` 保留，`th` 底边 2px，`tbody tr:nth-child(2n)>td` 加 `color-mix(fg 3.5%)` 斑马纹，行 hover `color-mix(accent 8%)`（同 NoteLume）。

### 5.9 状态栏（`statusbar.ts:38-89` + `styles.css:1281-1294`）

```
┌──────────────────────────────────────────────────────────────────────────┐
│ ● 未保存   UTF-8  LF  自动 │ 行 120/1240 · 列 8        [跟随尾部]  ⚠ 3    │ 28px
└──────────────────────────────────────────────────────────────────────────┘
```

- `#status`：`height:28px; padding: 0 12px; gap: 10px; background: var(--bg-panel); border-top:1px solid var(--border); font-size:12px; color: var(--text-2)`。
- `.status-label`（"编码/行尾/语法"）：11px、`--text-3`、`letter-spacing:.02em`；保留文字（可发现性）但降权。
- 三个 `select`：`.statusbar select` / `.statusbar .encoding/.eol/.mode` → `height:20px; padding: 0 4px; border:0; background: transparent; color: var(--text); border-radius: var(--r-xs)`；`hover` `--bg-hover`；`focus-visible` accent 描边。**class 名不动**（`statusbar.test.ts` 查 `.mode`）。
- `.status-follow`：chip 形态（20px、pill、hairline 边框）；active → `accent-soft` 底 + accent 文字。
- `.dirty-flag`（有 `.visible`）：前缀 6px 圆点用 `--accent`，文字"● 未保存"保留（测试与文案不依赖，但别改 DOM 文案）。
- `.doc-pos`：`font-variant-numeric: tabular-nums; margin-left:auto`。
- `.status-watch` `--warn`、`.status-invalid` `--warn` 保留。
- `.busy`：`opacity:.55; pointer-events:none`（当前 CSS 完全没有，需要补）。

### 5.10 弹窗 / 命令面板 / Toast / 恢复横幅

- 遮罩：`.palette-overlay / .modal-3way / .keymap-modal / .script-modal` → `background: var(--overlay)`；可选 `backdrop-filter: blur(2px)`，`@supports` 内启用（WebView2 支持）；`prefers-reduced-transparency: reduce` 时禁用。
- 弹窗盒子：`background: var(--bg-elevated); border:1px solid var(--border); border-radius: var(--r-lg); box-shadow: var(--shadow-3); padding: var(--sp-5)`；标题 15px/600；正文 13px/1.5 `--text-2`；操作行 `gap: var(--sp-2)`。
- 按钮：主按钮 `.btn-primary` → accent 实心、30px 高、6px 圆角、hover `filter: brightness(1.06)`；次按钮幽灵（`--bg-input` + hairline）。
- 命令面板 `.palette-box`：宽 560、`margin-top:12vh`、`padding: var(--sp-2)`、圆角 `--r-lg`；`.palette-input` 去边框、`height:36px`、`background: var(--bg-input)`、15px；`.palette-row` 34px、6px 圆角；选中 `.sel` 改为 `background: var(--accent-soft); color: var(--accent); box-shadow:none`（不再用左侧实心竖条）。
- Toast：`bottom:32px; padding: 9px 16px; border-radius: var(--r-pill); background: var(--bg-elevated); color: var(--text); border:1px solid var(--border); box-shadow: var(--shadow-2); font-size:12.5px`；进入 `opacity+translateY(6px)` `--dur-3`；退出 `--dur-2`。**不再反色黑条。**
- 恢复横幅 / 3-way / 外部改动条：统一 `--bg-elevated` + `--shadow-2` + `--r-md`；`ext-bar` 的 warn 边框改为 `border-left:3px solid var(--warn)`，其余边 hairline。
- 拖拽打开 `#drop-overlay`：`background: color-mix(accent 8%); border: 1.5px dashed var(--accent); color: var(--accent); font-size:14px; backdrop-filter: blur(2px)`。

### 5.11 基础控件基线（新增，位于 styles.css 顶部 token 之后）

```css
button, input, select, textarea { font: inherit; color: inherit; }
button {
  border: 1px solid transparent; background: transparent;
  border-radius: var(--r-sm); padding: 0 10px; height: var(--h-control);
  color: var(--text-2); cursor: default;
  transition: background-color var(--dur-1) var(--ease-out),
              color var(--dur-1) var(--ease-out),
              transform var(--dur-1) var(--ease-out);
}
button:hover { background: var(--bg-hover); color: var(--text); }
button:active { background: var(--bg-active); transform: scale(.97); }
button:disabled { opacity: .45; pointer-events: none; }
input, select, textarea {
  height: var(--h-control); padding: 0 8px;
  background: var(--bg-input); color: var(--text);
  border: 1px solid var(--border-strong); border-radius: var(--r-sm);
  outline: none;
}
input:focus, select:focus, textarea:focus {
  border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-soft);
}
:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
::selection { background: var(--selection); }
```

- 例外：状态栏下拉、标签内输入、`.tab` 等用类覆盖高度/边框（见各区域）。
- `.btn`（通用幽灵按钮）、`.btn-primary`（主操作）、`.chip`、`.field` 四个工具类，供新区域复用；旧类保留，不强行迁移。

---

## 6. 动效规范

| 场景 | 属性 | 时长/曲线 |
| --- | --- | --- |
| 按钮/行 hover | `background-color`, `color` | 90ms / ease-out |
| 按钮按下 | `transform: scale(.97)` | 90ms / ease-out |
| 面板展开（查询/dock） | `opacity 0→1`, `translateY(4px→0)` | 140ms / ease-out |
| 弹窗/命令面板 | `opacity 0→1`, `scale(.985→1)` | 200ms / ease-out |
| Toast | `opacity`, `translateY(6px→0)` | 200ms 进 / 140ms 出 |
| 折叠箭头（检查器/大纲） | `rotate(-90deg ↔ 0)` | 120ms / ease |
| 标签页切换 | 仅 `background-color`（不做滑条） | 90ms |
| 主题切换 | **不做**过渡（避免全屏闪色） | — |

禁止：位移超过 8px、spring/弹跳、`filter: blur` 动画、逐行 stagger。
`@media (prefers-reduced-motion: reduce)`：`* { transition-duration: 1ms !important; animation: none !important }`。

---

## 7. 实施顺序（每步可独立验收）

| 阶段 | 内容 | 文件 | 验收 |
| --- | --- | --- | --- |
| **P0 地基** | token 三主题重写 + 旧变量别名 + 基础控件基线 + 动效/圆角/间距 token + z-index | `styles.css:13-121` 段 | 三主题截图一致、`npm test` 全绿、`npm run build` |
| **P1 外壳** | 工具条 + 图标 sprite + 侧栏 + 标签条 + 检查器 | `index.html`、`main.ts:282-465`、`styles.css:123-256,769-897,985-1040` | 三栏骨架观感；侧栏/检查器拖拽仍正常；标签切换/关闭正常 |
| **P2 内容区** | CodeMirror chrome + 只读列表 + CAN/MF4/解析表 + dock 头 | `styles.css:1042-1134,1537-1656`、`canpanel.ts`（仅 class 增量） | 大文件滚动/虚拟化不回归（`ROW_HEIGHT` 未变）；表格对齐 |
| **P3 面板与状态** | 查询面板 + 状态栏 + 空态/降级态 | `styles.css:1096-1196,1281-1294`、`statusbar.ts`（class 不动） | 状态栏三下拉可用；`.mode/.status-follow/.status-watch` 测试通过 |
| **P4 浮层** | 命令面板 + 所有模态 + toast + 恢复/外部改动条 | `styles.css:319-536,1198-1314,1491-1535`、`main.ts` 内联 class | 打开/关闭动效；键盘 Esc/Enter 行为不变 |
| **P5 收尾** | focus-visible 全量扫、滚动条、`:disabled`、reduced-motion、空态文案 | `styles.css` | 键盘可达；无残留原生控件观感 |

每阶段结束：`cd app/frontend && npm test`；阶段 P0/P2/P4 额外 `npm run build`（含 `tsc --noEmit`）。Rust 未改动，最终只跑一次 `cargo check` 确认前端资源路径无影响。

---

## 8. 风险与硬约束

| 风险 | 说明 | 规避 |
| --- | --- | --- |
| **虚拟列表高度链** | `.listview` 的 `overflow:auto` 依赖 `#app → .workspace → .center → #tabs → .listview` 每层都有确定高度（`styles.css:7-11` 注释）。把 padding 加到错误的祖先层 → `clientHeight` 塌陷 → 一次请求整个文件 | 只在 `.listview` 内部加 `padding`（或在 `.list-text` 上加），**不改任何父级的 box 模型**；改完用只读大文件实测 |
| **`ROW_HEIGHT=20`** | `listview.test.ts:28` 断言 400px 视口 = 20 行；`ROW_HEIGHT` 同时驱动 spacer/transform | 本计划不改行高；若后续要 22，必须同改 JS 常量与测试并全量回归 |
| **表头/正文对齐** | `.can-hrow` 高度必须等于 `ROW_HEIGHT`（`styles.css:1610-1612`），列 flex 权重共用 | 只改 padding/颜色/字号；不改高度与 flex 比例 |
| **测试 class 契约** | `tabs.test.ts` 依赖 `.dirty-dot/.ext-dot/.visible/.warn`；`statusbar.test.ts` 依赖 `.mode/.status-watch/.status-follow`；`parserpanel.test.ts` 依赖 `.can-hrow/.can-row/.can-cell/.can-status`；`mf4-*`、`keymap-*`、`script-*` 同理 | 只**新增** class/结构，不重命名、不删除现有 class；`.dirty-dot` 保留 `visible` 开关语义 |
| **主题内联脚本** | `index.html:7-24` 在首帧前写 `data-theme` | 只改 token 值，不加新主题名；保留 `light/dark/vampire` 三个键 |
| **Tauri 无边框拖拽** | `titlebar-drag` 需要撑满条高 | 新工具条 40px 时保持 `align-self:stretch`，窗口按钮区不参与拖拽 |
| **`color-mix` 兼容** | 项目已在使用（`styles.css:801`），WebView2 支持 | 新增 token 同时给静态 rgba 兜底（`--accent-soft` 直接写 rgba） |
| **`prefers-reduced-transparency`** | 模糊遮罩在低性能设备上可能掉帧 | `@media (reduce)` 与 `@supports` 双层降级 |
| **改动面** | `main.ts` 158KB，其他任务可能并行 | 本分支独占；改动集中在 `layout()`、`refreshSidebar()`、`refreshPanels()`、`mkBtn()` 四处，不重构其他函数 |

**功能不动清单：** 命令/快捷键分派、tab 生命周期、IPC、Rust 端、虚拟化数学、编码/行尾切换语义、拖拽打开、恢复逻辑。**纯视觉改造，不改任何行为**（唯一行为增量：标签页新增关闭按钮，复用既有 `onClose`）。

---

## 9. 验证清单

1. `cd app/frontend && npm test` → 全绿（重点：tabs / statusbar / parserpanel / listview / mf4-ui / keymap-ui / script-ui）。
2. `npm run build` → `tsc --noEmit` + vite 通过。
3. `cd app && cargo check`（Rust 未改，仅确认资源链）。
4. 手工（dev 实例，新分支）：
   - 三主题切换，工具条/侧栏/检查器/状态栏/弹窗均无残留原生边框；
   - 拖拽两条分隔条，宽度范围仍是 160–480 / 200–520，刷新后保持；
   - 打开 >100MB 日志，滚动条拖动流畅、内存不暴涨（验证高度链）；
   - 报文面板表格与表头列对齐，滚到底部表头仍 sticky；
   - 命令面板 ↑↓ 选中、Esc 关闭；toast 4s 消失；
   - 键盘 Tab 走查：所有按钮/输入有可见 focus 环。
5. 截图对比：改造前/后各存 `%TEMP%`（不提交仓库，遵循仓库临时文件约束）。

---

## 10. 文件改动矩阵

| 文件 | 改动 | 规模 |
| --- | --- | --- |
| `app/frontend/src/styles.css` | token 重写；新增基线控件；按 §5 逐段替换 | 大（~60% 规则受影响，但多为覆盖值） |
| `app/frontend/index.html` | 新增 SVG sprite；主题脚本不动 | 小 |
| `app/frontend/src/main.ts` | `mkBtn` 加 icon；`layout()` 工具条分组；sidebar 空态/脚注 class；palette/toast 类名微调 | 中（约 80 行） |
| `app/frontend/src/tabs.ts` | 新增 `.tab-x` 关闭按钮（复用 `requestClose`） | 小（~10 行） |
| `app/frontend/src/statusbar.ts` | 不改 class/结构；仅新增 `.statusbar` 根类（当前只有 `className="statusbar"`，样式靠 `#status`） | 极小 |
| `app/frontend/src/listview.ts` | 不改（行高锁定）；如需行内 padding 由 CSS 完成 | 无 |
| `app/frontend/src/canpanel.ts` | 仅当 CSS 无法覆盖时加类；默认不动 | 无/极小 |
| `CHANGELOG.md` | 提交前追加条目（按仓库约定） | 小 |
