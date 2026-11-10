# LogLume 全功能黑盒测试报告（2026-11-10）

## 1. 范围与统计

| 项 | 值 |
|---|---|
| 模块数 | 20 |
| 用例数 | 165（`BB-001` ～ `BB-165`） |
| 每条用例属性 | 11 个 |
| 已填真实输出 | 47 |
| 用例质量门禁 | 通过 100/100，风险低，23 项检查与五项关键项全过，负例与边界 41%，平均步骤 2.3 |

## 2. 覆盖矩阵

| 模块 | 纳入的现有功能点 | 对应用例 |
|---|---|---|
| 文件与标签页 | 打开文件对话框、拖放打开、双击关联打开、启动带入文件、单实例转发、多标签切换与关闭、关闭未保存确认、左侧已打开文档列表、最近文件重开与清除、测量与报文与文档查看器的独立入口、另存为重绑标签 | `BB-001`～`007`、`120`～`122`、`136`～`139` |
| 编辑器与文本处理 | 基础编辑与撤销重做、剪切复制粘贴全选、内置语言高亮与按扩展名识别、状态栏切换语法、查找替换、跳转到行、九种行操作、行复制移动删除、自动换行、字号缩放、选中词高亮、列选择、三种格式化与压缩、中文右键菜单、状态栏行列与未保存标记 | `BB-008`～`016`、`140`～`142` |
| 编码与行尾 | 打开时自动识别编码、十三种编码手动切换、切换后整篇重新解码、保存与导出保持原编码、行尾自动识别与手动切换、混合行尾归一、不可映射字符提示、状态栏警告、文件头字节序标记剥除 | `BB-017`～`023`、`143`～`145` |
| 超大文件与只读查看 | 流式索引先首屏后校正行数、超过 200MB 只读虚拟列表、滚动行号正确、只读状态栏、只读下仍可过滤高亮与导出、跳到文件末尾、跟随尾部与上滚暂停、文件轮转回退重载、单文件 4GB 上限 | `BB-024`～`029`、`146` |
| 搜索过滤高亮替换 | 编辑器查找替换与命中间跳转、区分大小写与整词、多条件组合查询、过滤与高亮开关、命中跳转、流式扫描可停止、时序筛选与内置时间格式、行号区间筛选、非法正则空条件反转区间提示、目录级在文件中查找并点开定位、查找并替换预览与正则组引用 | `BB-030`～`039`、`147`～`149` |
| 保存冲突与导出 | 保存另存为、原子写入、外部改动冲突三选、目标被占用提示、只读文件拒绝、导出筛选片段、导出报文表格与逐帧文件、导出测量表格、导出各类文档文本、导出脚本结果与对话、导出进度与取消 | `BB-040`～`046`、`150`～`151` |
| CAN 报文与 DBC | 导入 DBC 严格与宽容模式、解析当前文档为报文、识别选中文本为报文、多种报文来源与帧类型、信号解码与物理值单位、报文虚拟表格、多轴组合过滤、报文与源行双向跳转、导出表格与逐帧文件、工具栏与数据菜单入口 | `BB-047`～`057`、`152`～`154` |
| 测量数据查看 | 打开测量文件显示元数据、通道搜索与多选、分页记录表与时间跳页、时间窗筛选、表格与波形两个标签、波形缩放平移与双击复位、无时间轴禁用波形、表格导出与进度、多会话上限 | `BB-058`～`062`、`155`～`156` |
| 文档格式查看器 | 表格文件工作表下拉与导出、文字文档段落提取、演示文档按页排序与分隔、PDF 渲染翻页与缩放、PDF 全文搜索与命中循环、文本层选择复制、缩略图跳页、文本层损坏仍可渲染 | `BB-063`～`068`、`157`～`158` |
| 脚本插件与扩展解析 | 脚本管理器新建保存删除、逐行脚本变换输出新文件、沙箱内存栈超时与输出上限、语法高亮插件与重载、逐行解析插件、Wasm 解析插件、内置分隔符表格、解析面板互斥显示 | `BB-069`～`074`、`159` |
| 版本管理 | 仓库探测与文件状态显示、初始化仓库、变更列表与暂存并提交、提交信息必填与作者引导、分支切换新建与名称校验、远端获取拉取推送与领先落后、合并冲突逐项选择与中止、刷新状态 | `BB-075`～`080`、`160` |
| 书签宏与编辑辅助 | 书签切换与环绕跳转、书签跨会话保存、宏录制播放与重复次数、运行到文件尾、已保存宏与键位绑定、概览条跳行、函数列表与过滤、三种格式化命令 | `BB-081`～`086`、`161` |
| 会话恢复与外部监听 | 崩溃恢复影子文件与启动横幅、恢复开关持久化、工作区会话静默重开、最近文件与文档标签、文档属性、外部修改自动重载、脏文档非阻断提示、删除改名提示、监听失败降级标记 | `BB-087`～`092`、`162` |
| 主题界面与状态栏 | 三套主题与跟随系统、主题偏好持久化、三栏布局与分栏拖拽、侧栏与信息栏显隐、自绘标题栏与原生窗口按钮、状态栏全部信息、状态栏右键入口、工具条与提示条 | `BB-093`～`098`、`163` |
| 菜单命令面板与右键 | 七个原生菜单分类、文件编辑查看数据设置窗口全部菜单项、工控工具下拉、命令面板模糊搜索全部动作与最近文件与宏、中文输入法合成保护、应用内中文右键菜单、键盘导航与退出键 | `BB-099`～`104` |
| 快捷键与自定义 | 六个默认绑定、十三个动作录制清除恢复、保留键与冲突键拒绝、启动时配置合并、宏键位与动作键位查重、固定上下文键、面板显示生效键位 | `BB-105`～`109`、`164` |
| AI 助手 | 默认关闭与引导卡片、预设服务商与自定义接口、地址格式校验与模型列表、四种对话模式、非日志文档禁用场景、发送前披露内容量与目标主机、流式输出可停止、上下文预算与截断披露、复制与新对话与导出 | `BB-110`～`114`、`165` |
| 设置与配置持久化 | 本机用户配置、浏览器本地存储、主题跟随系统、自动换行持久化、字体缩放仅会话级、侧栏免快捷键设置入口、崩溃恢复开关、界面不回显密钥明文 | `BB-115`～`119` |
| 窗口单实例与文件关联 | 单一窗口与无边框标题栏、原生菜单与覆盖式标题栏、全屏与最小化还原、单实例唤起并转发文件、启动带入文件、安装包注册关联、会话恢复与启动文件叠加顺序、跨平台与分辨率兼容 | `BB-120`～`124` |
| 附带的命令行工具 | 打开文件输出元信息与前若干行、只统计行数、关键字与正则搜索、按指定编码行尾保存、DBC 严格与宽松校验、解码报文并导出两种格式、生成合成测试日志、参数错误给出用法提示 | `BB-125`～`135` |

## 3. 真实执行输出

### 3.1 自动化回归

- 后端全量回归：`test result: ok. 388 passed; 0 failed; 1 ignored`（21 suites）
- 前端首轮：`Test Files  1 failed | 38 passed (39)` / `Tests  1 failed | 303 passed (304)` / `Duration  26.26s`
- 超时项单独复核：`✓ src/language.test.ts (14 tests) 574ms` / `Tests  14 passed (14)` / `Duration  3.26s`
- 类型检查：输出文件 0 字节
- 生产构建：`✓ 295 modules transformed.` / `✓ built in 26.72s`

### 3.2 桌面应用（macOS 15，前台运行）

- 菜单栏：`Apple, LogLume, 文件, 编辑, 查看, 数据, 设置, 窗口`
- 文件菜单：`打开文件…, 打开 MF4 测量数据…, 打开 BLF 报文记录…, 打开文档查看器…, missing value, 保存, 另存为…, missing value, 关闭当前文档, missing value, 清除最近文件记录`
- 编辑菜单：`撤销, 重做, missing value, 剪切, 复制, 粘贴, 全选, missing value`
- 查看菜单：`命令面板…, 行过滤 / 高亮命中…, 跳转到行…, missing value, 查找并替换（预览）…, 在文件中查找…, missing value, 切换自动换行, 放大字体, 缩小字体, 重置字体, missing value, 显示 / 隐藏侧栏, 显示 / 隐藏文档信息栏, missing value, 进入全屏`
- 数据菜单：`报文面板：显示 / 隐藏, 导入 DBC…, 解析当前文档为报文, 识别选中为报文, missing value, 导出报文 CSV…, 导出报文 JSONL…, missing value, 插件解析面板, 脚本管理器…`
- 设置菜单：`主题：浅色, 主题：暗黑, 主题：吸血鬼 (Dracula), 主题：跟随系统, missing value, 快捷键设置…`
- 窗口菜单：`最小化, 缩放, missing value, 前置全部窗口`
- 启动日志：`[menu] removed system item: Emoji & Symbols` ×3、`Start Dictation…` ×2、`AutoFill` ×1
- 命令面板：`osascript click exit=0`，截图 `输入命令…` / `打开文件…` / `保存` / `另存为…`，`pgrep 89489`
- 主题：`osascript click exit=0`，暗色界面与中文菜单栏，前台进程 `lumelog-app` pid 89489；浅色恢复截图同为 pid 89489
- 编辑器右键：`剪切/复制/粘贴/全选/查找/替换…/行操作/格式化/书签/AI/视图`，无 Reload 或 Inspect Element
- 剪贴板：`the clipboard as Unicode text → 台架 HIL 测试 记录 完成`；`clipboard info: «class utf8» 1269, «class ut16» 1900, string 25, Unicode text 1898`
- 打开文件：`sample_gbk.log` 标签、20 行中文日志、`编码 GBK / 行数 20 / 行尾 LF / 大小 1.1KB`、pid 89489
- 第二实例：`rc=0` 且无输出，首实例出现上述文档标签与元信息，pid 89489 未重启
- 窗口装饰：红黄绿窗口按钮、Overlay 标题栏、窗口内无 Windows 风格按钮
- 空白工作区：`control+click` 与 `AXShowMenu` 均无菜单

### 3.3 端到端输出

- C1 打开中文日志前 5 行：`{"size":1109,"lines":20,"encoding":"GBK","eol":"lf"}` / `2026-10-24 12:00:00 INFO 台架 HIL 测试 记录 完成 val=0` ～ `val=4` / `exit=0`
- C2 行数：`20` / `exit=0`
- C3 中文搜索：`{"matches":0,"scanned_bytes":1109,"cancelled":false,"truncated":false}` / `exit=0`
- C4 非法正则：`lumelog: invalid search pattern: regex parse error: ( / error: unclosed group` / `exit=1`
- C5 文件不存在：`lumelog: io error: No such file or directory (os error 2)` / `exit=1`
- C6 未知编码：`lumelog: unknown encoding 'NOPE'` + `usage: lumelog <open|search|save> --file PATH [options]` / `exit=2`
- C7 保存：`{"saved":true,"replace_count":0,"conflict":false}` / `exit=0`
- C8 重开：`{"size":14,"lines":2,"encoding":"GBK","eol":"crlf"}` / `中文回环 ABC` / `exit=0`
- A1 DBC 严格：`dbc: dbc syntax error at line 14 (bad SG_): SG_ BrokenSignal : 24|notanint@1+ (1,0) [0|1] "" Dashboard` / `exit=1`
- A2 CSV 导出：`line,ts,channel,id,dlc,data` / `1,1.234567,0,0x123,4,DEADBEEF` / `2,,0,0x735,4,11223344` / `3,,2,0x1F,9,A1B2C3D4E5F6070809` … / `# decoded=7 skipped=2` / `exit=0`
- A3 逐帧导出：`{"line":3,"ts":1,"channel":1,"id":"0x123","dlc":8,"data":"DEADBEEF00112233"}` / `{"line":4,"id":"0x1A7"}` / `{"line":5,"id":"0x321"}` / `{"decoded":3,"skipped":4}` / `exit=0`
- A4 日志不存在：`cannot read /tmp/no-such.log: No such file or directory (os error 2)` / `exit=1`
- A5 非法正则源：`decode: regex compile failed: regex parse error: ( / error: unclosed group` / `exit=2`
- A6 缺必填项：`decode: --log PATH is required` + usage / `exit=2`
- B1 DBC 宽松：`{"messages":3,"signals":6,"errors":1}` / `exit=0`
- C9 上限 3：`{"line":1,"id":"0x123","data":"DEADBEEF"}` / `{"line":2,"id":"0x735","data":"11223344"}` / `{"line":3,"id":"0x1F","data":"A1B2C3D4E5F6070809"}` / `{"decoded":7,"skipped":2}` / `exit=0`
- C10 上限 1：`{"line":1,"ts":1.234567,"channel":0,"id":"0x123","dlc":4,"data":"DEADBEEF"}` / `{"decoded":7,"skipped":2}` / `exit=0`
- C11 上限 0：`{"decoded":7,"skipped":2}` / `exit=0`
- D1 空搜索串：`exit=1`
- D2 非法上限：`decode: --limit must be a non-negative integer (got -1)` / `exit=2`
- D3 超长特殊字符：`exit=1`
- D4 无读权限：`lumelog: io error: Permission denied (os error 13)` / `exit=1`
- D5 重复保存：`{"saved":true,"replace_count":0,"conflict":false}` ×2 / `cmp=0`
- E1 空搜索串含错误输出：`lumelog: invalid search pattern: empty search pattern` / `exit=1`
- E2 超长串含错误输出：`lumelog: invalid search pattern: pattern is not representable in the file encoding` / `exit=1`
- F1 生成日志：`{"path":"/tmp/real-1mb.log","bytes":948516}` / `exit=0` / `bytes=948516`
- F2 搜索计时：`{"matches":16384,"scanned_bytes":948516,"cancelled":false,"truncated":true}` / `exit=0` / `real 0.35` / `user 0.08` / `sys 0.03`
- F3 行数计时：`16385` / `exit=0` / `real 0.31` / `user 0.06` / `sys 0.03`
- 并发：`direct_exit0=20/20` / `elapsed=0.07s` / `panic=0`；并发构建多轮 18～20/20，失败项 `rc=101 could not execute process target/debug/lumelog ... (never executed) / No such file or directory`
- 空文件：`{"size":0,"lines":1,"encoding":"UTF-8","eol":"lf"}` / `exit=0`
- 目录当文件：`lumelog: io error: Is a directory (os error 21)` / `exit=1`
- 带标记 UTF-16 打开：`{"size":20,"lines":3,"encoding":"UTF-16 LE","eol":"lf"}` / `中文测试` / `第二行` / `exit=0`
- 带标记 UTF-16 搜索：`{"line":0,"byte_in_line":0,"len":6,"snippet":"中文测试"}` / `{"matches":1,"scanned_bytes":20,"cancelled":false,"truncated":false}` / `exit=0`
- 非 ASCII 报文：`{"decoded":0,"skipped":1}` / `exit=0`

## 4. 用例质量门禁

- 第一轮 135 条：不通过，94/100，风险高；`F2 负例与边界占比 ≥ 40%` 实测 28%，其余 22 项通过。
- 补 30 条负例与边界场景：`BB-136` ～ `BB-165`。
- 第二轮 165 条：通过，100/100，风险低；23 项检查与五项关键项全过，五维均 100 分，负例与边界 41%，平均步骤 2.3。
