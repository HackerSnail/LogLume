# 多光标、列选择与多光标宏（Plan 47）

编辑器支持 Alt+左键拖拽的矩形选区、Ctrl+左键点击的多光标，并把多光标编辑录进宏。
全部前端改动，Rust 零改动。

## 1. 为什么先要开 allowMultipleSelections

`rectangularSelection()` 产生的矩形选区是**每行一个 range 的多 range 选区**；
`@codemirror/state` 在事务更新时对未启用 `EditorState.allowMultipleSelections`
的选区做 `asSingle()` 塌缩，多出来的 range 一进状态就只剩 main 那条。表现上
与普通拖拽完全一样：Alt+拖拽只得到一条普通选区，Ctrl+点击也加不出第二个光标。

`editor.ts` 的 `buildState` 里补 `EditorState.allowMultipleSelections.of(true)`
后，矩形选区与多光标才真正落地：

- Alt+左键拖拽 → 矩形选区（每行一个 range）；
- Ctrl+左键点击 → 追加一个光标，再点同一处 → 移除该光标（CM 原生语义，
  非 mac 上 `addsSelectionRange` 判 `event.ctrlKey`）；
- Ctrl+左键拖拽 → 追加一段新选区（Windows 下 `dragMovesSelection` 默认
  `!event.ctrlKey`，按 Ctrl 时不移动选区而是扩 range）。

状态栏在多光标时追加「N 个光标」，主光标的行列号照旧。

## 2. 宏录制：光标集与多光标编辑

Plan 30 的操作模型是相对光标的四类动作，重放都作用在 `selection.main`，
多光标场景下会录成"主光标的垃圾"。Plan 32 把多选区事务整段跳过，等于
多光标编辑完全不进宏。

新增两类操作：

- `{ kind: "cursors", spots }` —— 光标集。每个 spot 是
  `{fromLine, fromCol, toLine, toCol}`（1-based 行、0-based 列）。用行列
  而不是绝对偏移，前面发生偏移变化时录制结果仍然可读。
- `{ kind: "multi-edit", edits }` —— 各光标互不相同的编辑。每个 edit 是
  `{from, to, text}`，`from`/`to` 是相对该光标 head 的偏移（`from ≤ 0`、
  `to ≥ 0`）。

录制规则：

- 选区光标数 >1（或从 >1 回到 1）时，把当前光标集写成一条 `cursors` 操作；
  纯单光标点击仍然不记（普通点击不是宏步骤）；
- 多光标下的编辑事务按光标顺序折叠：各光标的改动完全一致时（多光标打字、
  粘贴、Backspace 都是这种情况）落成单光标那套 `insert`/`delete` 操作；
  不一致时（例如一个光标带选区、另一个没有）落成 `multi-edit`。

重放规则：

- `insert` / `delete` / `del-word` / `move` 改为作用到**所有**光标
  （CM 的 `replaceSelection`、`deleteGroupBackward` 等命令本身就按多选区处理）；
- `cursors` 把选区设回录制的光标集（行列换算成偏移，按行长钳制）；
- `multi-edit` 按光标顺序逐个套用相对编辑；
- `line`（行批处理）仍只作用于主选区，属已知限制。

## 3. 验收

- `editor.test.ts`：多光标录制→重放（打字、Backspace）、各光标不同的编辑、
  从多光标回到单光标各一条；
- `editortools.test.ts`：新操作类型的序列化往返 + 坏数据丢弃；
- `statusbar.test.ts`：单光标不显示计数，多光标显示「N 个光标」；
- 真实窗口 CDP 复测：Ctrl+点击加光标 → 状态栏「2 个光标」→ 录制 →
  两处同时插入 → 重置文档 → 播放宏 → 两处再次插入。
