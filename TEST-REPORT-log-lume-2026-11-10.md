# log-lume（澄章 / LogLume）测试用例与执行报告

- 被测对象：`/Users/cc/Downloads/项目/log-lume`（版本 0.1.0，workspace：`lumelog-core` / `lumelog-can` / `lumelog-app` + `app/frontend`）
- 执行日期：2026-11-10　执行人：AI 测试（CodeBuddy / test-mentor 链路）
- 执行环境：macOS（darwin，Apple Silicon）· Node v24.14.0 · npm 11.9.0 · cargo/rustc 1.99.0 · 被测二进制 `target/debug/lumelog`、`target/debug/lumelog-can`、`target/debug/lumelog-gen`（仓库已有 debug 构建产物）
- 夹具目录：`/tmp/loglume-tc-20261110`（**未向仓库写入任何测试数据，未修改仓库任何代码**）；执行脚本：`run_cli.sh`、`run_rust_core_can.sh`、`run_rust_app.sh`、`fe_run.sh`、`fe_rerun.sh`（同目录）
- 口径说明：CLI 行号为 0 基；`rc` 为进程退出码（0 成功 / 1 引擎或 IO 错误 / 2 用法错误）；"测试结果/实际结果"仅在**真实跑过**时填写，未执行的写"未测试"及原因

## 0. 执行概览

| 执行面 | 方式 | 规模 | 结果 |
|---|---|---|---|
| CLI 黑盒（`lumelog` / `lumelog-can`） | 逐条命令 + 落盘校验 | 99 次命令 | 94 条用例，5 条不通过 |
| Rust 核心/引擎（`lumelog-core` / `lumelog-can`） | `cargo test --exact` 逐条精确执行 | 41 条精确用例 + 全量 | 全部 passed（core 123 / can 25 / app 184 套件） |
| Rust 桌面壳（`lumelog-app`） | `cargo test --exact` 逐条精确执行 | 52 条精确用例 | 全部 passed |
| 前端（Vitest） | `vitest run <file> -t "<name>"` 逐条 + 全量 | 42 条逐条（39 文件 314 用例） | 全部 passed（首轮 16 条因 `-t` 名称未匹配未运行，重跑后均 1 passed） |
| 构建与集成 | `tsc --noEmit`、`vite build`、`cargo test --workspace`、ACL/配置静态核对 | 6 条 | 全部 passed |
| GUI 交互（Tauri 桌面窗口） | 需要交互桌面会话 | 6 条 | 未测试（本环境无可交互 GUI 会话） |

用例总数 **143**（类型分布：正常 36 / 边界 43 / 异常 33 / 越界 12 / 空值 5 / 权限 5 / 特殊字符 4 / 网络 3 / 超长 2；**异常 + 边界 + 越界 + 空值 + 超长 = 95 条，占 66.4%**），未通过 **5** 条，未测试 **6** 条。

---

## 1. 用例表

### 1.1 文档与编码（DOC）— CLI `lumelog open|save`

| 编号 | 模块 | 前置条件 | 测试环境 | 测试数据 | 测试步骤 | 测试要点 | 预期结果 | 用例级别 | 测试结果 | 实际结果 |
|---|---|---|---|---|---|---|---|---|---|---|
| DOC-01 | 文档与编码 | 夹具已生成 | CLI-L · macOS · debug 构建 | `utf8.log`，33B，`台架 HIL 测试 OK\n温度=25.0\n` | 1. `lumelog open --file utf8.log`<br>2. 读 stdout JSON | 正常：UTF-8 自动识别、字节数、行数、行尾 | rc=0，`{"size":33,"lines":3,"encoding":"UTF-8","eol":"lf"}` | P0 | 通过 | rc=0；`{"size":33,"lines":3,"encoding":"UTF-8","eol":"lf"}` |
| DOC-02 | 文档与编码 | 同上 | 同上 | `gbk.log`，45B，GBK `动力系统控制台台架测试\n错误 记录: 传感器异常\n` | `lumelog open --file gbk.log` | 正常：GBK 自动识别不乱码 | rc=0，`encoding:"GBK"`，size=45，lines=3 | P0 | 通过 | rc=0；`{"size":45,"lines":3,"encoding":"GBK","eol":"lf"}` |
| DOC-03 | 文档与编码 | 同上 | 同上 | `utf8bom.log`，`EF BB BF` + `BOM 文件第一行\n第二行\n` | `lumelog open --file utf8bom.log --head 1` | 正常：BOM 识别与剥除 | rc=0，`encoding:"UTF-8 BOM"`，首行不含 U+FEFF | P1 | 通过 | rc=0；`{"size":33,"lines":3,"encoding":"UTF-8 BOM",...}`，首行 `BOM 文件第一行` |
| DOC-04 | 文档与编码 | 同上 | 同上 | `utf16le_bom.log`，`FF FE` BOM + `UTF16LE BOM 第一行\n第二行\n` | `lumelog open --file utf16le_bom.log --head 2` | 正常：UTF-16 LE 双端序解码 | rc=0，`encoding:"UTF-16 LE"`，首行中文正确 | P1 | 通过 | rc=0；`{"size":42,"lines":3,"encoding":"UTF-16 LE","eol":"lf"}`，中文正常 |
| DOC-05 | 文档与编码 | 同上 | 同上 | `utf16be_bom.log`，`FE FF` BOM + `UTF16BE BOM 第一行\n第二行\n` | `lumelog open --file utf16be_bom.log --head 2` | 正常：UTF-16 BE 解码 | rc=0，`encoding:"UTF-16 BE"`，中文正确 | P1 | 通过 | rc=0；`{"size":42,"lines":3,"encoding":"UTF-16 BE","eol":"lf"}`，中文正常 |
| DOC-06 | 文档与编码 | 同上；文件**无 BOM** | 同上 | `utf16le.log`，32B，UTF-16LE 无 BOM（`55 00 54 00 …`，无 `FF FE`） | `lumelog open --file utf16le.log --head 2` | 异常：无 BOM 的 UTF-16 自动识别 | 应识别为 UTF-16 LE 并正确显示中文 | P1 | 不通过 | rc=0；`encoding:"ISO-8859-1"`，输出乱码（`UTF16LE ,{NLˆ`），中文全错 |
| DOC-07 | 文档与编码 | 同上；文件**无 BOM** | 同上 | `utf16be.log`，32B，UTF-16BE 无 BOM | `lumelog open --file utf16be.log --head 2` | 异常：无 BOM 的 UTF-16BE 自动识别 | 应识别为 UTF-16 BE | P1 | 不通过 | rc=0；`encoding:"GBK"`，输出乱码（`UTF16BE {,N圠`） |
| DOC-08 | 文档与编码 | 同上；手动指定编码 | 同上 | `utf16le.log`（无 BOM，32B）+ `--encoding "UTF-16 LE"` | 1. `lumelog open --file utf16le.log --encoding "UTF-16 LE" --head 2`<br>2. `od -c` 复核行尾字节 | 异常：手动切换 UTF-16 后按行取块，行尾不得出现替换符 | 输出行内容无 U+FFFD | P1 | 不通过 | 第 2 行末尾多出 `EF BF BD`（U+FFFD）；同样内容带 BOM 时（对照）字节干净无替换符 |
| DOC-09 | 文档与编码 | 夹具已生成 | 同上 | `big5.log`，96B，Big5 繁体中文 2 行 | `lumelog open --file big5.log --head 1` | 正常：Big5 自动识别（与 GBK 字节区间重叠） | rc=0，`encoding:"BIG5"`，首行正确 | P1 | 通过 | rc=0；`encoding:"BIG5"`，首行 `繁體中文測試日誌 錯誤 溫度感測器異常 2026-10-30` |
| DOC-10 | 文档与编码 | 同上 | 同上 | `sjis.log`，114B，Shift-JIS 日文 2 行 | `lumelog open --file sjis.log --head 1` | 正常：Shift-JIS 自动识别 | rc=0，`encoding:"Shift-JIS"`，日文正确 | P1 | 通过 | rc=0；`encoding:"Shift-JIS"`，日文正确 |
| DOC-11 | 文档与编码 | 同上 | 同上 | `eucjp.log`，114B，EUC-JP 日文 2 行 | `lumelog open --file eucjp.log --head 1` | 正常：EUC-JP 自动识别 | rc=0，`encoding:"EUC-JP"` | P1 | 通过 | rc=0；`encoding:"EUC-JP"`，日文正确 |
| DOC-12 | 文档与编码 | 同上 | 同上 | `euckr.log`，112B，EUC-KR 韩文 2 行 | `lumelog open --file euckr.log --head 1` | 正常：EUC-KR 自动识别 | rc=0，`encoding:"EUC-KR"` | P1 | 通过 | rc=0；`encoding:"EUC-KR"`，韩文正确 |
| DOC-13 | 文档与编码 | 同上 | 同上 | `koi8r.log`，72B，KOI8-R 俄文 2 行 | `lumelog open --file koi8r.log --head 1` | 正常：KOI8 系列识别 | rc=0，识别为 KOI8-U 且不乱码 | P1 | 通过 | rc=0；`encoding:"KOI8-U"`，俄文正确 |
| DOC-14 | 文档与编码 | 同上 | 同上 | `gb18030_4byte.log`，24B，含 𠀀（U+20000，4 字节） | `lumelog open --file gb18030_4byte.log --head 1` | 边界：GB18030 四字节生僻字 | rc=0，生僻字正确显示 | P1 | 通过 | rc=0；`encoding:"GBK"`（标签非 GB18030），首行 `生僻字 𠀀 测试` 正确 |
| DOC-15 | 文档与编码 | 同上 | 同上 | `empty.log`，0 字节 | `lumelog open --file empty.log` | 边界：空文件 | rc=0，`size:0`，不报错 | P2 | 通过 | rc=0；`{"size":0,"lines":1,"encoding":"UTF-8","eol":"lf"}` |
| DOC-16 | 文档与编码 | 同上 | 同上 | `crlf.log`，14B，`line1\r\nline2\r\n` | `lumelog open --file crlf.log` | 正常：CRLF 行尾识别 | rc=0，`eol:"crlf"` | P1 | 通过 | rc=0；`{"size":14,"lines":3,"encoding":"UTF-8","eol":"crlf"}` |
| DOC-17 | 文档与编码 | 同上 | 同上 | `mixed.log`，8B，`a\r\nb\nc\r\n` | `lumelog open --file mixed.log` | 边界：混合行尾 | rc=0，`eol:"mixed"` | P1 | 通过 | rc=0；`{"size":8,"lines":4,...,"eol":"mixed"}` |
| DOC-18 | 文档与编码 | 同上 | 同上 | `no_trailing_nl.log`，3B，`a\nb` | `lumelog open --file no_trailing_nl.log --line-count` | 边界：无尾换行 | 行数=2 | P1 | 通过 | rc=0；输出 `2` |
| DOC-19 | 文档与编码 | 同上 | 同上 | `crlf.log` | `lumelog open --file crlf.log --head 2` | 边界：CRLF 输出不得夹带 CR | 两行输出均不含 `\r` | P2 | 通过 | rc=0；输出 `line1` / `line2`，无 `\r` |
| DOC-20 | 文档与编码 | 同上 | 同上 | `--file ""`（空路径） | `lumelog open --file "" --line-count` | 空值：空路径 | rc=2，明确指出 `--file` 不能为空 | P2 | 通过 | rc=2；stderr `lumelog: --file must not be empty` + USAGE，stdout 为空 |
| DOC-21 | 文档与编码 | 同上 | 同上 | `--encoding UTF-32` | `lumelog open --file utf8.log --encoding UTF-32` | 异常：未知编码 | rc=2，`unknown encoding 'UTF-32'` | P1 | 通过 | rc=2；stderr `unknown encoding 'UTF-32'`，stdout 空 |
| DOC-22 | 文档与编码 | 同上 | 同上 | `--encoding ""` | `lumelog open --file utf8.log --encoding ""` | 空值：空编码 | rc=2，`unknown encoding ''` | P2 | 通过 | rc=2；stderr `unknown encoding ''` |
| DOC-23 | 文档与编码 | 同上 | 同上 | `--eol DOS`（stdin=`x\n`） | `printf 'x\n' \| lumelog save --file save_lf.log --eol DOS --stdin` | 异常：未知行尾 | rc=2，`unknown --eol 'dos'` | P1 | 通过 | rc=2；stderr `unknown --eol 'dos'` |
| DOC-24 | 文档与编码 | 同上 | 同上 | `--head abc` | `lumelog open --file tail.log --head abc` | 异常：非法行数 | rc=2，且不输出半截摘要 JSON | P2 | 通过 | rc=2；stderr `--head must be a number`，stdout 空 |
| DOC-25 | 文档与编码 | 同上 | 同上 | `--head 0`（下边界） | `lumelog open --file tail.log --head 0` | 边界：行数下边界 | rc=0，只输出 JSON 摘要，不打印正文 | P2 | 通过 | rc=0；仅 `{"size":6212,"lines":202,...}` |
| DOC-26 | 文档与编码 | 同上 | 同上 | `--head 1000000`（上边界，文件仅 202 行） | `lumelog open --file tail.log --head 1000000` | 边界：行数上边界 | rc=0，打印全部 202 行 | P2 | 通过 | rc=0；打印全部 202 行 + 摘要 |
| DOC-27 | 文档与编码 | 同上 | 同上 | `--head 99999999999999999999`（超 u64） | `lumelog open --file utf8.log --head 99999999999999999999` | 越界：行数溢出 | rc=2，提示数值非法 | P2 | 通过 | rc=2；stderr `--head must be a number` |
| DOC-28 | 文档与编码 | 同上 | 同上 | `nope.log`（不存在） | `lumelog open --file nope.log` | 异常：文件不存在 | rc=1，报找不到文件，不创建文件 | P2 | 通过 | rc=1；stderr `io error: No such file or directory (os error 2)` |
| DOC-29 | 文档与编码 | 同上 | 同上 | 目录 `/tmp/loglume-tc-20261110` | `lumelog open --file /tmp/loglume-tc-20261110` | 异常：目标是目录 | rc=1，报拒绝访问/非普通文件 | P2 | 通过 | rc=1；stderr `io error: Is a directory (os error 21)` |
| DOC-30 | 文档与编码 | 同上 | 同上 | `invalid_gbk.log`，4B，`CA 0A 78 0A`（悬挂 GBK 前导字节） | `lumelog open --file invalid_gbk.log --head 2` | 异常：非法字节序列 | rc=0，不 panic，以 U+FFFD 替换 | P1 | 通过 | rc=0；`encoding:"GB18030"`，首行含 `�`，第二行 `x`，无 panic |
| DOC-31 | 文档与编码 | 磁盘可写稀疏文件 | 同上 | `huge_4gib.log`，4,294,967,296B（u32::MAX+1） | 1. `f.truncate(4294967296)`<br>2. `lumelog open --file huge_4gib.log` | 越界：超过 4GiB 上限 | rc=1，`file too large`，不建索引 | P1 | 通过 | rc=1；stderr `file too large (exceeds 4GB u32 index limit or configured size limit)`，耗时 <1s |
| DOC-32 | 文档与编码 | 同上 | 同上 | `size_max.log`，4,294,967,295B（恰为 u32::MAX，上边界） | `lumelog open --file size_max.log --line-count` | 边界：4GiB 上限内的最大文件 | rc=0，正常建立行索引 | P1 | 通过 | rc=0；输出行数 `1`（全零字节无换行） |
| DOC-33 | 文档与编码 | 同上 | 同上 | `longline.log`，10,485,768B（10MB `A` + `\nneedle\n`） | `lumelog open --file longline.log --line-count` | 超长：10MB 单行 | rc=0，行数=3，不 OOM | P2 | 通过 | rc=0；输出 `3` |
| DOC-34 | 文档与编码 | 同上 | 同上 | `unicode_name.log`（文件名 `日志 2026 😀 report.log`） | `lumelog open --file "…/日志 2026 😀 report.log"` | 特殊字符：中文+空格+emoji 文件名 | rc=0，正常打开 | P2 | 通过 | rc=0；`{"size":24,"lines":2,"encoding":"UTF-8","eol":"lf"}` |
| DOC-35 | 文档与编码 | 同上 | 同上 | `special.log`，88B：`<script>alert(1)</script>` / `needle[]()$.*+?^|` / `' OR 1=1 --` / `😀 中文 "quoted" \ back<TAB>tab` | `lumelog open --file special.log --head 4` | 特殊字符：正文原样输出不执行 | rc=0，4 行原样输出 | P2 | 通过 | rc=0；4 行原样输出（脚本标签、SQL 片段、emoji、引号、反斜杠、Tab 均原样） |
| DOC-36 | 文档与编码 | 夹具有读权限 | 同上 | `acl_denied.log` = `utf8.log` 副本，`chmod 000` | 1. `chmod 000 acl_denied.log`<br>2. `lumelog open --file acl_denied.log`<br>3. `chmod 644` 复原 | 权限：读权限被拒 | rc=1，报 Permission denied | P1 | 通过 | rc=1；stderr `io error: Permission denied (os error 13)` |
| DOC-37 | 文档与编码 | 夹具可写 | 同上 | `ro2.log` = `utf8.log` 副本，`chmod 444`（只读），stdin=`new content\n` | 1. `chmod 444 ro2.log`<br>2. `printf 'new content\n' \| lumelog save --file ro2.log --stdin`<br>3. `od -c ro2.log` 复核 | 权限：只读文件保存应被拒绝且内容不变 | rc=1，报只读/权限错误，文件字节不变 | P1 | 不通过 | rc=0，`{"saved":true,...}`；文件内容被改写为 `new content\n`（只读位被绕过） |
| DOC-38 | 文档与编码 | 目录可读不可写 | 同上 | `nowrite/f.log`，`chmod 555 nowrite` | 1. `chmod 555 nowrite`<br>2. `printf 'x\n' \| lumelog save --file nowrite/f.log --stdin`<br>3. `chmod 755` 复原 | 权限：目录不可写时保存 | rc=1，报 Permission denied，原文件不变 | P1 | 通过 | rc=1；stderr `io error: Permission denied (os error 13)`；`od -c` 显示内容仍为原 UTF-8 文本 |
| DOC-39 | 文档与编码 | 无该主机/共享 | 同上 | `//no-such-host-lumelog/share/x.log` | `lumelog open --file //no-such-host-lumelog/share/x.log` | 网络：不可达网络路径快速失败 | rc=1，3s 内返回，不挂起 | P2 | 通过 | rc=1；`No such file or directory (os error 2)`，即时返回 |
| DOC-40 | 文档与编码 | 无该挂载卷 | 同上 | `/Volumes/no-such-volume-lumelog/x.log` | `lumelog open --file /Volumes/no-such-volume-lumelog/x.log` | 网络：不可达挂载点 | rc=1，快速失败 | P2 | 通过 | rc=1；`No such file or directory (os error 2)`，即时返回 |
| DOC-41 | 文档与编码 | 夹具可写 | 同上 | `save_lf.log`（原 `old\n`），stdin=`new1\nnew2\n` | `printf 'new1\nnew2\n' \| lumelog save --file save_lf.log --stdin` | 正常：保存主流程 | rc=0，`saved:true`，落盘 `new1\nnew2\n` | P0 | 通过 | rc=0；`{"saved":true,"replace_count":0,"conflict":false}`；落盘 `n e w 1 \n n e w 2 \n` |
| DOC-42 | 文档与编码 | 同上 | 同上 | `save_crlf.log`（原 `old\r\n`），stdin=`a\nb\n` | `printf 'a\nb\n' \| lumelog save --file save_crlf.log --eol CRLF --stdin` | 正常：保存保持 CRLF | rc=0，落盘 `a\r\nb\r\n` | P0 | 通过 | rc=0；`saved:true`；落盘 `a \r \n b \r \n` |
| DOC-43 | 文档与编码 | 夹具为 GBK | 同上 | `save_gbk.log`（GBK `原始内容\n`），stdin=`ok😀\n` | `printf 'ok😀\n' \| lumelog save --file save_gbk.log --encoding GBK --stdin` | 异常：目标编码不可映射字符 | rc=1，提示不可映射字符数，文件字节不变 | P1 | 通过 | rc=1；stderr `1 characters are unmappable in the target encoding`；`od -c` 仍为原 GBK 字节 `324 255 312 274 304 332 310 335` |
| DOC-44 | 文档与编码 | 目标文件已存在 | 同上 | `empty_save.log`（已存在），stdin 为空 | `printf '' \| lumelog save --file empty_save.log --stdin` | 空值：空输入保存 | rc=0，落盘 0 字节 | P2 | 通过 | rc=0；`saved:true`；文件大小 0 |
| DOC-45 | 文档与编码 | 源文件已存在 | 同上 | `src_save.log`，stdin=`out1\nout2\n`，`--out saved_new.log` | `printf 'out1\nout2\n' \| lumelog save --file src_save.log --out saved_new.log --stdin` | 正常：另存为 | rc=0，新文件=`out1\nout2\n`，源文件不变 | P1 | 通过 | rc=0；`saved:true`；`saved_new.log`=`out1\nout2\n`；`src_save.log` 字节未变 |
| DOC-46 | 文档与编码 | 夹具已生成 | 同上 | `gbk.log` + `--encoding GB2312` | `lumelog open --file gbk.log --encoding GB2312` | 正常：编码别名 | rc=0，GB2312 映射为 GBK | P1 | 通过 | rc=0；`{"size":45,"lines":3,"encoding":"GBK",...}` |
| DOC-47 | 文档与编码 | 同上 | 同上 | 无参数 / 缺 `--pattern` / 缺 `--file` | 1. `lumelog`<br>2. `lumelog search --file utf8.log`<br>3. `lumelog save` | 异常：缺少必填参数 | 三次 rc=2，打印 USAGE | P2 | 通过 | 三次 rc=2；分别为 USAGE、`missing --pattern`、`missing --file` |
| DOC-48 | 文档与编码 | 同上 | 同上 | `lumelog open --file utf8.log --help` | `lumelog open --file utf8.log --help` | 异常：帮助选项 | 应返回帮助文本（rc=0） | P2 | 不通过 | rc=2；stderr `unknown option '--help' for subcommand 'open'`（`lumelog` 无 `--help`，而 `lumelog-can help` 可用） |

### 1.2 搜索与检索（SRC）— CLI `lumelog search`

| 编号 | 模块 | 前置条件 | 测试环境 | 测试数据 | 测试步骤 | 测试要点 | 预期结果 | 用例级别 | 测试结果 | 实际结果 |
|---|---|---|---|---|---|---|---|---|---|---|
| SRC-01 | 搜索与检索 | 夹具已生成 | CLI-L · macOS | `gbk.log`，pattern=`错误` | `lumelog search --file gbk.log --pattern 错误 --max 10` | 正常：GBK 中文搜索与字节偏移 | rc=0，命中 1 处，`len:4` | P0 | 通过 | rc=0；`{"line":1,"byte_in_line":0,"len":4,"snippet":"错误 记录: 传感器异常"}` + `{"matches":1,"scanned_bytes":45,...}` |
| SRC-02 | 搜索与检索 | 同上 | 同上 | `tail.log`（202 行 / 201 处 INFO），pattern=`val=9[0-9][0-9]`，`--regex --max 5` | `lumelog search --file tail.log --pattern "val=9[0-9][0-9]" --regex --max 5` | 正常：正则搜索 + 截断 | rc=0，命中总数 201，`truncated:true`，只打印 5 条 | P1 | 通过 | rc=0；打印 5 条命中，`{"matches":201,"scanned_bytes":6212,"cancelled":false,"truncated":true}` |
| SRC-03 | 搜索与检索 | 同上 | 同上 | `--max 0`（下边界） | `lumelog search --file tail.log --pattern INFO --max 0` | 边界：命中上限下边界 | rc=0，不打印命中行但统计 `matches:201` | P2 | 通过 | rc=0；仅统计行 `{"matches":201,...,"truncated":true}` |
| SRC-04 | 搜索与检索 | 同上 | 同上 | `--max 5`（上边界截断） | `lumelog search --file tail.log --pattern INFO --max 5` | 边界：命中上限截断 | 恰好打印 5 条 | P2 | 通过 | rc=0；打印 5 条 + `matches:201`、`truncated:true` |
| SRC-05 | 搜索与检索 | 同上 | 同上 | 空 pattern | `lumelog search --file tail.log --pattern ""` | 空值：空搜索串 | rc=1，报 empty search pattern | P1 | 通过 | rc=1；stderr `invalid search pattern: empty search pattern` |
| SRC-06 | 搜索与检索 | 同上 | 同上 | 非法正则 `(` + `--regex` | `lumelog search --file tail.log --pattern "(" --regex` | 异常：正则编译失败 | rc=1，报 regex parse error | P1 | 通过 | rc=1；stderr `regex parse error: … unclosed group` |
| SRC-07 | 搜索与检索 | 同上 | 同上 | `gbk.log` + pattern=`😀` | `lumelog search --file gbk.log --pattern 😀` | 异常：pattern 在文件编码中不可表示 | rc=1，明确报错 | P1 | 通过 | rc=1；stderr `pattern is not representable in the file encoding` |
| SRC-08 | 搜索与检索 | 同上 | 同上 | `--mode fuzzy`（未知选项） | `lumelog search --file tail.log --pattern INFO --mode fuzzy` | 异常：未知选项必须被拒 | rc=2，`unknown option '--mode'`，不执行搜索 | P1 | 通过 | rc=2；stderr `unknown option '--mode' for subcommand 'search'`，无命中输出 |
| SRC-09 | 搜索与检索 | 同上 | 同上 | `utf16le.log`（自动识别为 ISO-8859-1）+ pattern=`第一行` | `lumelog search --file utf16le.log --pattern 第一行` | 异常：UTF-16 文件搜索明确拒绝 | rc=1，提示 v1 不支持 | P1 | 通过 | rc=1；stderr `invalid search pattern: pattern is not representable in the file encoding` |
| SRC-10 | 搜索与检索 | 同上 | 同上 | `u16_nobom_cjk.log` + `--encoding "UTF-16 LE"` + pattern=`第一行` | `lumelog search --file u16_nobom_cjk.log --encoding "UTF-16 LE" --pattern 第一行` | 异常：UTF-16 编码下查询拒绝 | rc=1，`UTF-16 search unsupported in v1` | P1 | 通过 | rc=1；stderr `encoding error: UTF-16 search unsupported in v1` |
| SRC-11 | 搜索与检索 | 同上 | 同上 | `empty.log`（0 字节）+ pattern=`x` | `lumelog search --file empty.log --pattern x` | 边界：空文件搜索 | rc=0，`matches:0`、`scanned_bytes:0` | P1 | 通过 | rc=0；`{"matches":0,"scanned_bytes":0,"cancelled":false,"truncated":false}` |
| SRC-12 | 搜索与检索 | 同上 | 同上 | `--max -1`（越界负值） | `lumelog search --file tail.log --pattern INFO --max -1` | 越界：命中上限为负 | rc=2，提示非负整数，无命中输出 | P1 | 通过 | rc=2；stderr `--max must be a non-negative integer` |
| SRC-13 | 搜索与检索 | 同上 | 同上 | `--max 18446744073709551616`（u64 溢出） | `lumelog search --file tail.log --pattern INFO --max 18446744073709551616` | 越界：数值溢出 | rc=2，同样提示 | P1 | 通过 | rc=2；stderr `--max must be a non-negative integer` |
| SRC-14 | 搜索与检索 | 同上 | 同上 | 未知子命令 `frobnicate` | `lumelog frobnicate --file utf8.log` | 异常：未知子命令 | rc=2，`unknown subcommand 'frobnicate'` | P2 | 通过 | rc=2；stderr `unknown subcommand 'frobnicate'` + USAGE |
| SRC-15 | 搜索与检索 | 同上 | 同上 | `seam.log`：`A`×8,388,605 + `SEAMNEEDLE`，跨 8MiB 窗口边界 | `lumelog search --file seam.log --pattern SEAMNEEDLE --max 5` | 边界：跨块接缝不重不漏 | 恰好 1 次命中，`byte_in_line:8388605` | P1 | 通过 | rc=0；`{"line":0,"byte_in_line":8388605,"len":10,...}`，`matches:1` |
| SRC-16 | 搜索与检索 | 同上 | 同上 | `special.log` + pattern=`needle[]()$.*+?^|` | `lumelog search --file special.log --pattern "needle[]()$.*+?^|"` | 特殊字符：正则元字符按字面处理 | rc=0，命中第 2 行，`len:17` | P2 | 通过 | rc=0；`{"line":1,"byte_in_line":0,"len":17,"snippet":"needle[]()$.*+?^|"}`，`matches:1` |
| SRC-17 | 搜索与检索 | 同上 | 同上 | `special.log` + pattern=`' OR 1=1 --` | `lumelog search --file special.log --pattern "' OR 1=1 --"` | 特殊字符：SQL 片段按字面处理 | rc=0，字面命中 | P2 | 通过 | rc=0；命中该行，`len:11`，未做任何注入行为 |
| SRC-18 | 搜索与检索 | 同上 | 同上 | pattern=`$(rm -rf /); \`id\` && echo x` | `lumelog search --file special.log --pattern '$(rm -rf /); `id` && echo x'` | 特殊字符：命令注入样式输入不被执行 | rc=0，`matches:0`，无副作用 | P2 | 通过 | rc=0；`{"matches":0,"scanned_bytes":88,...}`，未执行任何外部命令 |
| SRC-19 | 搜索与检索 | 同上 | 同上 | `longline.log`（10MB 单行） + pattern=`needle` | `lumelog search --file longline.log --pattern needle --max 3` | 超长：超长行内搜索 | rc=0，命中 1 处 | P2 | 通过 | rc=0；`{"line":1,"byte_in_line":0,"len":6,...}`，`scanned_bytes:10485768` |
| SRC-20 | 搜索与检索 | 同上 | 同上 | `tail.log` + `--ignore-case` + pattern=`info` | `lumelog search --file tail.log --pattern info --ignore-case --max 3` | 正常：忽略大小写 | rc=0，命中 201 处 | P1 | 通过 | rc=0；打印 3 条，`matches:201`、`truncated:true` |
| SRC-21 | 搜索与检索 | 大文件已生成 | 同上 | `bench_64mb.log`，60,701,996B / 1,048,577 行 | 1. `/usr/bin/time -p lumelog open --file bench_64mb.log --line-count`<br>2. `/usr/bin/time -p lumelog search --file bench_64mb.log --pattern "val=999" --max 20` | 正常：大文件全量索引 + 全文搜索 | 行数正确，搜索在 10s 内完成 | P0 | 通过 | 行数 `1048577`（0.19s）；搜索 `matches:1013`、`scanned_bytes:60701996`、`truncated:true`（0.31s） |
| SRC-22 | 搜索与检索 | 同上 | 同上 | `empty.log` + 空 pattern + `--regex` | `lumelog search --file empty.log --pattern "" --regex` | 空值：空 pattern 叠加正则 | rc=1，报空搜索串 | P2 | 通过 | rc=1；stderr `invalid search pattern: empty search pattern` |

### 1.3 CAN / DBC（CAN）— CLI `lumelog-can`

| 编号 | 模块 | 前置条件 | 测试环境 | 测试数据 | 测试步骤 | 测试要点 | 预期结果 | 用例级别 | 测试结果 | 实际结果 |
|---|---|---|---|---|---|---|---|---|---|---|
| CAN-01 | CAN/DBC | 仓库夹具 `tests/data/sample.dbc`（第 14 行坏 `SG_`） | CLI-C · macOS | `SG_ BrokenSignal : 24\|notanint@1+ …` | `lumelog-can dbc --file sample.dbc` | 异常：严格模式遇坏行 | rc=1，指出第 14 行语法错误 | P0 | 通过 | rc=1；stderr `dbc syntax error at line 14 (bad SG_): SG_ BrokenSignal : 24\|notanint@1+ …` |
| CAN-02 | CAN/DBC | 同上 | 同上 | 同上 + `--lenient` | `lumelog-can dbc --file sample.dbc --lenient` | 正常：宽松模式跳过坏行 | rc=0，`messages:3`、`signals:6`、`errors:1` | P0 | 通过 | rc=0；`{"messages":3,"signals":6,"errors":1}` |
| CAN-03 | CAN/DBC | 夹具已生成 | 同上 | `can/empty.dbc`，0 字节 | `lumelog-can dbc --file can/empty.dbc` | 边界：空 DBC | rc=0，`0/0/0`，不报错 | P1 | 通过 | rc=0；`{"messages":0,"signals":0,"errors":0}` |
| CAN-04 | CAN/DBC | 同上 | 同上 | `can/no_such.dbc`（不存在） | `lumelog-can dbc --file can/no_such.dbc` | 异常：DBC 文件不存在 | rc=1，明确报错 | P1 | 通过 | rc=1；stderr `dbc: cannot read …/no_such.dbc` |
| CAN-05 | CAN/DBC | 同上 | 同上 | `can/bad.dbc`：坏 `BO_ notanumber` + 孤立 `SG_` + 1 条合法消息 | `lumelog-can dbc --file can/bad.dbc --lenient` | 异常：多类非法行不阻断其它消息 | rc=0，坏行计入 errors | P1 | 通过 | rc=0；`{"messages":2,"signals":3,"errors":1}` |
| CAN-06 | CAN/DBC | 仓库夹具 `candump_basic.log` + `sample.dbc` | 同上 | 10 行 candump（含 RTR、扩展 ID、CAN FD、坏 hex、非帧行） | `lumelog-can decode --log candump_basic.log --format candump --dbc sample.dbc --out jsonl` | 正常：candump 解析 + DBC 解码 | rc=0，`decoded:7`、`skipped:2`；EngineSpeed raw 44510 / physical 5563.75 | P0 | 通过 | rc=0；`{"decoded":7,"skipped":2}`；0x123 帧 `EngineSpeed raw=44510 physical=5563.75 rpm`、`CoolantTemp raw=-66 physical=-106` |
| CAN-07 | CAN/DBC | 仓库夹具 `cli_intel.log` + `bitorder.dbc` | 同上 | `can0 64#DEADBEEF12345678` | `lumelog-can decode --log cli_intel.log --format candump --dbc bitorder.dbc --out jsonl` | 正常：Intel 位序信号解码 | I_U8=222、I_S16=-16723/-8371.5、I_U20=266991 | P0 | 通过 | rc=0；`I_U8 raw=222 physical=222`、`I_S16 raw=-16723 physical=-8371.5`、`I_U20 raw=266991`、`I_U4 raw=3 physical=0.6` |
| CAN-08 | CAN/DBC | 仓库夹具 `asc_basic.asc` | 同上 | 8 行 ASC（含 `Rx d`、`Tx r`、`boe`、空行） | `lumelog-can decode --log asc_basic.asc --format asc --out csv` | 正常：ASC 源 + CSV 导出 | rc=0，表头 + 3 行，`decoded=3 skipped=4` | P1 | 通过 | rc=0；stdout `line,ts,channel,id,dlc,data` + 3 行；stderr `# decoded=3 skipped=4` |
| CAN-09 | CAN/DBC | 夹具 `can/regex_ok.log` | 同上 | `T=12.345 ID=0x100 DATA=11223344` 等 2 行 | `lumelog-can decode --log can/regex_ok.log --format regex --regex "T=(?P<ts>[0-9.]+) ID=(?P<id>0x[0-9A-Fa-f]+) DATA=(?P<data>[0-9A-Fa-f]+)" --out jsonl` | 正常：自定义正则源 | rc=0，2 帧，ts 12.345/13.7 | P1 | 通过 | rc=0；`{"line":1,"ts":12.345,...,"id":"0x100","data":"11223344"}`、`{"line":2,"ts":13.7,...,"id":"0x200"}`，`decoded:2 skipped:0` |
| CAN-10 | CAN/DBC | 同上 | 同上 | 缺命名组的正则 `T=(?P<t>[0-9.]+)` | `lumelog-can decode --log can/regex_ok.log --format regex --regex "T=(?P<t>[0-9.]+)"` | 异常：正则缺 `id`/`data` 组 | rc=2，明确提示必须定义的组 | P1 | 通过 | rc=2；stderr `invalid regex: pattern must define named groups \`id\` and \`data\` (optional \`ts\`)` |
| CAN-11 | CAN/DBC | 同上 | 同上 | 非法正则 `(` | `lumelog-can decode --log can/regex_ok.log --format regex --regex "("` | 异常：正则编译失败 | rc=2，报编译错误 | P1 | 通过 | rc=2；stderr `regex compile failed: regex parse error: … unclosed group` |
| CAN-12 | CAN/DBC | 夹具 `can/candump_bad.log` | 同上 | 奇数 hex(`456#ABC`)、非 hex(`789#ZZZZ`)、空 `can0`、非帧行、1 条合法帧 | `lumelog-can decode --log can/candump_bad.log --format candump --out jsonl` | 异常：多路坏帧 | rc=0，合法帧仍解出，其余计 skipped | P1 | 通过 | rc=0；`{"line":1,...,"data":"DEADBEEF"}` + `{"decoded":1,"skipped":4}` |
| CAN-13 | CAN/DBC | 夹具 `can/emoji_payload.log` | 同上 | `can0 123#😀` | `lumelog-can decode --log can/emoji_payload.log --format candump --out jsonl` | 特殊字符：非 ASCII payload 不 panic | rc=0，不 panic，坏帧计 skipped | P0 | 通过 | rc=0；`{"decoded":0,"skipped":1}`，无 panic |
| CAN-14 | CAN/DBC | 夹具 `can/rtr_ext_fd.log` | 同上 | `can0 123#R`、`can0 1FFFFFFF#42`、`can0 123##031122334455667788` | `lumelog-can decode --log can/rtr_ext_fd.log --format candump --out jsonl` | 边界：RTR 帧 / 扩展 ID / CAN FD | 3 帧全部解出，RTR dlc=0 | P1 | 通过 | rc=0；3 行输出（dlc 0 / 1 / 8），`decoded:3 skipped:0` |
| CAN-15 | CAN/DBC | 夹具 `can/p65.log` | 同上 | `can0 123#` + `AB`×65（65 字节，超 64 上限） | `lumelog-can decode --log can/p65.log --format candump --out jsonl` | 越界：payload 超过 64 字节 | 帧被拒并计 skipped，不 panic | P1 | 通过 | rc=0；`{"decoded":0,"skipped":1}`，无 panic |
| CAN-16 | CAN/DBC | 夹具 `empty.log` | 同上 | 0 字节日志 | `lumelog-can decode --log empty.log --format candump --out jsonl` | 边界：空日志 | rc=0，`decoded:0 skipped:0` | P2 | 通过 | rc=0；`{"decoded":0,"skipped":0}` |
| CAN-17 | CAN/DBC | 仓库夹具 `candump_basic.log` | 同上 | `--limit 0 --out jsonl` | `lumelog-can decode --log candump_basic.log --format candump --limit 0 --out jsonl` | 边界：命中上限下界为 0 | 只输出统计，不打印帧 | P2 | 不通过 | rc=0；仍打印全部 8 帧 JSON + `{"decoded":7,"skipped":2}`，`--limit` 被忽略 |
| CAN-18 | CAN/DBC | 同上 | 同上 | `--limit abc` | `lumelog-can decode --log candump_basic.log --format candump --limit abc --out jsonl` | 异常：非数字上限 | rc=2，提示非负整数 | P2 | 通过 | rc=2；stderr `--limit must be a non-negative integer (got abc)` |
| CAN-19 | CAN/DBC | 同上 | 同上 | `--format xml` | `lumelog-can decode --log candump_basic.log --format xml --out jsonl` | 异常：非法枚举值 | rc=2，列出合法取值 | P2 | 通过 | rc=2；stderr `--format must be candump\|asc\|regex (got xml)` |
| CAN-20 | CAN/DBC | 同上 | 同上 | 缺 `--log` | `lumelog-can decode --format candump --out jsonl` | 异常：缺少必填参数 | rc=2，提示 `--log PATH is required` | P2 | 通过 | rc=2；stderr `decode: --log PATH is required` |
| CAN-21 | CAN/DBC | 同上 | 同上 | `--out xml` | `lumelog-can decode --log candump_basic.log --format candump --out xml` | 异常：非法输出格式 | rc=2 | P2 | 通过 | rc=2；stderr `--out must be jsonl or csv (got xml)` |
| CAN-22 | CAN/DBC | 夹具 `can/asc_bad.asc` | 同上 | 1 合法帧 + 1 坏行 + 1 bogus 行 | `lumelog-can decode --log can/asc_bad.asc --format asc --out jsonl` | 异常：ASC 坏行跳过 | rc=0，合法帧解出，坏行计 skipped | P1 | 通过 | rc=0；2 帧输出（0x123 dlc8、0x1A7 dlc4），`{"decoded":2,"skipped":2}` |
| CAN-23 | CAN/DBC | 仓库夹具 `sample.dbc`（含扩展帧 0x80000000） | 同上 | `can0 500#1122334455667788` | `lumelog-can decode --log <ext_frame> --format candump --dbc sample.dbc --out jsonl` | 正常：扩展帧 ID 掩码匹配 DBC | 命中 ExtFrame 并解码信号 | P1 | 通过 | rc=0；扩展 ID 帧按掩码命中 `ExtFrame` 并输出 `SignalA` 原始值/物理值 |
| CAN-24 | CAN/DBC | Rust 单测可执行 | cargo test · macOS | `blf::tests::bad_magic_is_error` / `truncated_container_is_error` / `classic_dlc_over_8_is_clamped` | `cargo test -p lumelog-can blf::tests:: -- --exact`（逐条） | 异常/边界：BLF 坏 magic、截断容器、DLC>8 | 各用例 passed，不 panic | P1 | 通过 | 3 条逐条执行均 `1 passed; 0 failed` |
| CAN-25 | CAN/DBC | Rust 单测可执行 | cargo test · macOS | `dbc_golden::signal_length_out_of_range_is_rejected` / `lenient_skips_bad_line_records_context` | `cargo test -p lumelog-can --test dbc_golden <name> -- --exact` | 异常：信号位宽越界、坏行上下文 | 各用例 passed | P1 | 通过 | 2 条逐条执行均 `1 passed; 0 failed` |
| CAN-26 | CAN/DBC | Rust 单测可执行 | cargo test · macOS | `decode_proptest::overlong_signal_does_not_panic` / `physical_is_linear` / `intel_matches_reference` | `cargo test -p lumelog-can --test decode_proptest <name> -- --exact` | 边界/属性：超长信号不 panic、Intel 位序参考实现 | 各用例 passed | P1 | 通过 | 3 条逐条执行均 `1 passed; 0 failed` |
| CAN-27 | CAN/DBC | Rust 单测可执行 | cargo test · macOS | `regex_source::missing_required_group_errors` / `invalid_regex_compile_errors` / `odd_or_invalid_hex_data_is_skipped` | `cargo test -p lumelog-can --test regex_source <name> -- --exact` | 异常：正则缺组/编译失败/非法 hex | 各用例 passed | P1 | 通过 | 3 条逐条执行均 `1 passed; 0 failed` |
| CAN-28 | CAN/DBC | Rust 集成测试可执行 | cargo test · macOS | `cli_can::missing_file_exits_1` / `bad_format_exits_2` / `dbc_summary_counts_messages` / `decode_with_dbc_emits_signal_physical` / `decodes_100k_frames_faithfully`；`candump::remote_dlc_over_64_is_skipped`；`asc::skips_boe_bcs_error_counted` | `cargo test -p lumelog-can --test cli_can --test candump --test asc <name> -- --exact`（逐条） | 正常/异常：CLI 退出码、10 万帧保真、DLC 越界、BOE/BCS 跳过 | 各用例 passed | P1 | 通过 | 7 条逐条执行均 `1 passed; 0 failed`（含 10 万帧解码用例） |

### 1.4 核心引擎（CORE）— `cargo test -p lumelog-core` 精确用例逐条

| 编号 | 模块 | 前置条件 | 测试环境 | 测试数据 | 测试步骤 | 测试要点 | 预期结果 | 用例级别 | 测试结果 | 实际结果 |
|---|---|---|---|---|---|---|---|---|---|---|
| CORE-01 | 核心引擎·行过滤 | Rust 工具链可用 | cargo test · lumelog-core | `filter::tests::empty_pattern_rejected` / `invalid_regex_rejected` / `exclude_vetoes` / `literal_metachar_is_literal` / `utf16_offsets_for_multibyte` | 逐条 `cargo test -p lumelog-core <name> -- --exact` | 异常/边界：空条件、坏正则、排除否决、元字符字面、UTF-16 多字节偏移 | 5 条全部 passed | P1 | 通过 | 5 条逐条执行均 `1 passed; 0 failed` |
| CORE-02 | 核心引擎·导出 | 同上 | cargo test · lumelog-core | `export::tests::export_selected_lines_byte_parity` / `export_cancel_stops_early` / `export_all_preserves_exact_bytes_crlf_no_trailing_newline` | 逐条精确执行 | 正常/边界/异常：按字节导出、取消、CRLF 无尾换行保真 | 3 条全部 passed | P1 | 通过 | 3 条逐条执行均 `1 passed; 0 failed` |
| CORE-03 | 核心引擎·CSV | 同上 | cargo test · lumelog-core | `csv::tests::quoted_sections` / `tab_and_pipe_delimiters` / `plain_and_empty_fields` | 逐条精确执行 | 边界：RFC4180 引号段、多分隔符、空字段 | 3 条全部 passed | P1 | 通过 | 3 条逐条执行均 `1 passed; 0 failed` |
| CORE-04 | 核心引擎·目录查找 | 同上 | cargo test · lumelog-core | `fif::tests::literal_and_case_modes_over_mixed_encodings` | `cargo test -p lumelog-core fif::tests::literal_and_case_modes_over_mixed_encodings -- --exact` | 正常：混合编码目录检索、大小写模式 | passed | P1 | 通过 | `1 passed; 0 failed` |
| CORE-05 | 核心引擎·搜索 | 同上 | cargo test · lumelog-core | `search::tests::dense_matches_across_seam_equal_whole_file` / `utf16_query_is_rejected` / `unmappable_literal_pattern_rejected` / `line_number_correct` | 逐条精确执行 | 边界/异常：接缝密集命中、UTF-16 查询拒绝、不可表示 pattern、行号正确 | 4 条全部 passed | P1 | 通过 | 4 条逐条执行均 `1 passed; 0 failed` |
| CORE-06 | 核心引擎·编码探测 | 同上 | cargo test · lumelog-core | `encoding::tests::detect_bom_variants` | 精确执行 | 边界：BOM 变体（UTF-8/16LE/16BE/32）探测 | passed | P1 | 通过 | `1 passed; 0 failed` |
| CORE-07 | 核心引擎·文档打开 | 同上 | cargo test · lumelog-core | `docfile`：`open_rejects_file_over_size_limit` / `file_shrink_during_read_is_error_not_panic` / `interior_bom_is_preserved_as_content` / `utf16le_open_uses_aligned_index` / `chunk_at_zero_lines_returns_empty_not_panic` | `cargo test -p lumelog-core --test docfile <name> -- --exact`（逐条） | 越界/异常：超大文件、读时文件缩小、内嵌 BOM、UTF-16 对齐索引、零行取块 | 5 条全部 passed，不 panic | P1 | 通过 | 5 条逐条执行均 `1 passed; 0 failed` |
| CORE-08 | 核心引擎·CLI 冒烟 | 同上 | cargo test · lumelog-core | `cli_smoke`：`bad_flag_value_is_usage_exit_2` / `negative_max_reports_range_error` / `save_roundtrips_gbk_from_stdin` / `head_on_crlf_file_drops_carriage_return` | `cargo test -p lumelog-core --test cli_smoke <name> -- --exact`（逐条） | 异常/边界：非法参数退出码 2、负数范围、GBK 往返、CRLF 去 CR | 4 条全部 passed | P1 | 通过 | 4 条逐条执行均 `1 passed; 0 failed` |
| CORE-09 | 核心引擎·属性测试 | 同上 | cargo test · lumelog-core | `encoding_proptest::incremental_split_anywhere_matches_whole`、`chunk_proptest` | `cargo test -p lumelog-core --test encoding_proptest --test chunk_proptest` | 边界：任意切分点增量解码等价 | passed | P2 | 通过 | `1 passed; 0 failed`（encoding_proptest） |

### 1.5 桌面壳（APP）— `cargo test -p lumelog-app` 精确用例逐条

| 编号 | 模块 | 前置条件 | 测试环境 | 测试数据 | 测试步骤 | 测试要点 | 预期结果 | 用例级别 | 测试结果 | 实际结果 |
|---|---|---|---|---|---|---|---|---|---|---|
| APP-01 | AI 助手·配置校验 | Rust 工具链可用 | cargo test · lumelog-app | `ai::tests::defaults_are_off_and_empty` / `enable_requires_endpoint_model_and_key` / `rejects_bad_urls_and_temperature` / `maps_http_status_to_stable_codes` / `empty_key_is_removed_not_written` / `loopback_enable_needs_no_key` / `truncation_never_splits_multibyte_chars` | 逐条 `cargo test -p lumelog-app <name> -- --exact` | 异常/边界：默认关闭、启用缺项、非法 URL、温度 0–2 越界、状态码映射、Key 不落盘、回环免 Key、多字节截断 | 10 条全部 passed | P1 | 通过 | 10 条逐条执行均 `1 passed; 0 failed` |
| APP-02 | AI 助手·网络异常 | 单测内置本地 HTTP 服务 | cargo test · lumelog-app | `ai::stream_tests::http_401_maps_to_ai_auth` / `http_429_maps_to_rate_limited` / `cancel_after_first_delta_stops_the_stream` | 逐条精确执行 | 网络异常：鉴权失败、限流、流取消 | 3 条全部 passed | P1 | 通过 | 3 条逐条执行均 `1 passed; 0 failed` |
| APP-03 | IPC 命令 | 同上 | cargo test · lumelog-app | `commands::source_validation_tests::regex_invalid_pattern_rejected` / `unknown_kind_rejected`；`filter_run_tests::scan_filter_cancel_stops_early` / `scan_filter_truncates_over_cap`；`doc_lines_at_tests::gather_lines_over_cap_rejects`；`can_import_tests::dbc_import_missing_file_is_io_error`；`tail_tests::tail_append_grows_then_no_change`；`reload_tests::reload_unknown_doc_is_unknown_doc`；`export_cmd_tests::open_export_target_bad_path_is_io` | 逐条精确执行 | 异常/边界：来源校验、过滤取消与截断、超限拒绝、缺失文件、tail 增长、未知文档、导出目标非法 | 9 条全部 passed | P1 | 通过 | 9 条逐条执行均 `1 passed; 0 failed` |
| APP-04 | 报文导出 | 同上 | cargo test · lumelog-app | `can_export::tests::id_reversed_matches_none` / `time_window_no_ts_drops` / `export_cancel_stops_and_is_prefix` / `csv_row_escapes_and_handles_no_msg` / `export_csv_includes_dbc_signal_columns` | 逐条精确执行 | 边界/异常：反向 ID 区间为空、无时间戳剔除、导出取消前缀、CSV 转义与空消息、信号列 | 5 条全部 passed | P1 | 通过 | 5 条逐条执行均 `1 passed; 0 failed` |
| APP-05 | 崩溃恢复 / 最近文件 | 同上 | cargo test · lumelog-app | `recovery::tests::cfg_default_true_when_missing_or_bad` / `discard_idempotent` / `list_sorted_and_skips_bad`；`recent::tests::record_dedups_to_front_and_caps` | 逐条精确执行 | 异常/边界：配置缺失或损坏回落默认、丢弃幂等、坏数据跳过、最近文件去重与上限 | 4 条全部 passed | P1 | 通过 | 4 条逐条执行均 `1 passed; 0 failed` |
| APP-06 | QuickJS 脚本 | 同上 | cargo test · lumelog-app | `script::run_tests::keep_replace_drop` / `js_throw_stops_with_prefix` / `bad_return_type_errors`；`script::store_tests::validate_name_cases` / `console_caps_messages_and_count` | 逐条精确执行 | 正常/异常/边界：三种返回值、脚本抛错前缀、返回类型非法、脚本名 1–64 字符、控制台 200 条/4096 字符上限 | 5 条全部 passed | P1 | 通过 | 5 条逐条执行均 `1 passed; 0 failed` |
| APP-07 | 插件 / WASM | 同上 | cargo test · lumelog-app | `plugins::tests::limits_are_enforced` / `broken_json_does_not_block_others`；`wasmi_host::tests::wasm_empty_plugin_and_broken_module` | 逐条精确执行 | 边界/异常：插件 64 文件/256KB/16 扩展/512 关键词/32 规则上限、坏 manifest 隔离、0 字节与坏 wasm 拒绝 | 3 条全部 passed | P1 | 通过 | 3 条逐条执行均 `1 passed; 0 failed` |
| APP-08 | 查看器 / MF4 | 同上 | cargo test · lumelog-app | `viewers::tests::xlsx_open_rows_and_formatting` / `unsupported_extension_and_session_cap` / `pdf_text_extraction_and_malformed_input` / `pptx_without_slides_is_rejected`；`mf4::tests::series_buckets_envelope_and_limits` | 逐条精确执行 | 正常/异常/边界：xlsx 打开导出、不支持格式与会话上限 4、损坏 PDF、空 pptx、MF4 序列与桶上限 | 5 条全部 passed | P1 | 通过 | 5 条逐条执行均 `1 passed; 0 failed` |
| APP-09 | Git 面板 | 同上（含本地 bare 仓库） | cargo test · lumelog-app | `gitinfo::tests::status_log_diff_and_commit_all` / `remote_add_edit_and_validation` / `identity_resolution_prefers_explicit_then_config_then_error` / `fetch_push_and_remote_status` / `conflicts_can_take_a_side_or_abort` | 逐条精确执行 | 正常/异常：暂存提交、非法分支/远端名与 URL、身份缺失、fetch/push、冲突择边 | 5 条全部 passed | P1 | 通过 | 5 条逐条执行均 `1 passed; 0 failed` |
| APP-10 | 外部修改监听 | 同上 | cargo test · lumelog-app | `watch::tests::classify_size_or_mtime_changed` / `classify_missing_removed` / `watch_status_starts_not_degraded` | 逐条精确执行 | 异常/边界：size/mtime 变化、文件消失、监听状态 | 3 条全部 passed | P1 | 通过 | 3 条逐条执行均 `1 passed; 0 failed` |

### 1.6 前端（FE）— Vitest 逐条执行

| 编号 | 模块 | 前置条件 | 测试环境 | 测试数据 | 测试步骤 | 测试要点 | 预期结果 | 用例级别 | 测试结果 | 实际结果 |
|---|---|---|---|---|---|---|---|---|---|---|
| FE-01 | 编辑器·行操作 | `app/frontend` 依赖已安装 | Vitest 2 · Node 24 · jsdom | `editorops.test.ts`（14 用例） | `npx vitest run src/editorops.test.ts` | 正常：9 种行操作且不改入参 | 全部 passed | P1 | 通过 | `applyLineOp does not mutate the input` 逐条 1 passed；文件整体 14/14 |
| FE-02 | 编辑器·行操作 | 同上 | 同上 | `editorops.test.ts` 字号钳制 | `npx vitest run src/editorops.test.ts -t "font size"` | 边界：字号 9–28 上下界与越界 | passed | P2 | 通过 | 1 passed |
| FE-03 | 格式化 | 同上 | 同上 | `formatters.test.ts`（8 用例）：非法 JSON `{oops`、非法 XML `<a><b></a>` | `npx vitest run src/formatters.test.ts` | 异常：格式化失败不改文本 | 全部 passed | P1 | 通过 | `JSON formats and minifies`、`XML formats elements…` 逐条 1 passed；8/8 |
| FE-04 | 替换工具 | 同上 | 同上 | `replacetools.test.ts`（11 用例）：空查找串、非法正则 `(`、零长匹配 | `npx vitest run src/replacetools.test.ts` | 异常/边界：替换构造与零长匹配不死循环 | 全部 passed | P1 | 通过 | `buildReplaceRe escapes literal metacharacters`、`findMatches skips zero-length matches without looping` 逐条 1 passed；11/11 |
| FE-05 | 快捷键 | 同上 | 同上 | `keymap.test.ts`（11 用例）：裸键 `S`、`F3`、`Shift+F3` | `npx vitest run src/keymap.test.ts -t "hijack"` | 权限/保留键：不可劫持输入的键位 | passed | P1 | 通过 | `rejects bindings that would hijack typing or shadow context keys` 1 passed |
| FE-06 | 快捷键设置 UI | 同上 | 同上 | `keymap-ui.test.ts`（2 用例） | `npx vitest run src/keymap-ui.test.ts -t "records a valid key"` | 异常：冲突键位与裸键被拒 | passed | P1 | 通过 | 1 passed |
| FE-07 | 查询面板 | 同上 | 同上 | `query.test.ts`（7 用例）：`from=5,to=""`、`from=9,to=2`、`from=a,to=5` | `npx vitest run src/query.test.ts -t "line_range_requires_both_sides"` / `-t "line_range_reversed_errors"` | 异常：行号区间双侧必填、反向区间 | 2 条 passed | P1 | 通过 | 两条逐条各 1 passed |
| FE-08 | 主题 | 同上 | 同上 | `theme.test.ts`（4 用例）：light/dark/vampire/auto | `npx vitest run src/theme.test.ts -t "auto follows the system dark flag"` | 正常/边界：auto 跟随系统 | passed | P2 | 通过 | 1 passed |
| FE-09 | 只读列表 | 同上 | 同上 | `listview.test.ts`（14 用例）：`totalLines=0`、窗口 `first>last` | `npx vitest run src/listview.test.ts -t "empty document yields an empty window"` / `-t "isNearBottom"` | 边界：空文档窗口为空、tail 阈值 48px | 2 条 passed | P2 | 通过 | 逐条执行：`empty document…` 1 passed；`isNearBottom` 3 passed |
| FE-10 | 语法高亮 | 同上 | 同上 | `language.test.ts`（14 用例）：70+ 模式 id、插件模式 `plugin:<id>`、坏正则规则 | `npx vitest run src/language.test.ts -t "resolves every selectable mode"` / `-t "drops rules whose regex does not compile"` | 边界/异常：注册表漂移守卫、坏规则跳过 | 2 条 passed | P2 | 通过 | 逐条各 1 passed |
| FE-11 | AI 上下文预算 | 同上 | 同上 | `aitasks.test.ts`（10 用例）：200 行 / 40000 字符上限、50000 条命中 | `npx vitest run src/aitasks.test.ts -t "clamps by character budget without dropping everything"` / `-t "discloses a scan that stopped at the sample cap"` | 边界/超长：上下文截断并披露 | 2 条 passed | P1 | 通过 | 逐条各 1 passed |
| FE-12 | AI 设置 | 同上 | 同上 | `aisettings.test.ts`（5 用例）：`https://api.example.com/v1`、`sk-test-123456` | `npx vitest run src/aisettings.test.ts -t "never renders the key"` / `-t "keeps an untouched key"` | 权限/敏感信息：Key 只写不回显 | 2 条 passed | P0 | 通过 | 逐条各 1 passed |
| FE-13 | AI 面板 | 同上 | 同上 | `aipanel.test.ts`（10 用例）：默认关闭态 | `npx vitest run src/aipanel.test.ts -t "disables every egress control"` | 权限：未启用时无任何外发控件 | passed | P1 | 通过 | 1 passed |
| FE-14 | 报文过滤 | 同上 | 同上 | `canfilter.test.ts`（7 用例）：`idFrom:0x1FF,idTo:0x100`、缺时间戳帧 | `npx vitest run src/canfilter.test.ts -t "id_range_inclusive"` / `-t "time_window_drops_ts_none"` | 边界：反向区间为空、时间窗剔除无时间戳帧 | 2 条 passed | P2 | 通过 | 逐条各 1 passed |
| FE-15 | 报文面板 | 同上 | 同上 | `canpanel.test.ts`（3 用例）：两批 push + clear | `npx vitest run src/canpanel.test.ts -t "frame_store_revision"` | 正常：分批累积不丢帧、revision 递增 | passed | P2 | 通过 | 1 passed |
| FE-16 | 会话恢复 | 同上 | 同上 | `session.test.ts`（4 用例）：60 条路径 | `npx vitest run src/session.test.ts -t "caps at 50"` | 边界：会话 50 上限、去重、空值丢弃 | passed | P2 | 通过 | 1 passed |
| FE-17 | 书签 | 同上 | 同上 | `bookmarks-store.test.ts`（4 用例）：201 个书签 | `npx vitest run src/bookmarks-store.test.ts -t "caps lines per document"` | 边界：每文档 200 行上限 | passed | P2 | 通过 | 1 passed |
| FE-18 | 标签 | 同上 | 同上 | `doctags.test.ts`（6 用例）：21 标签 / 33 字符 | `npx vitest run src/doctags.test.ts -t "caps tags per document"` | 边界：标签 20×32 上限 | passed | P2 | 通过 | 1 passed |
| FE-19 | 宏与编辑工具 | 同上 | 同上 | `editortools.test.ts`（20 用例）：重复次数 0/-1/abc/10001 | `npx vitest run src/editortools.test.ts -t "parseRepeatCount"` / `-t "round-trips through serializeMacros"` | 边界/异常：次数 1–10000、宏存储上限 | 2 条 passed | P2 | 通过 | `parseRepeatCount` 2 passed；宏往返 1 passed |
| FE-20 | PDF 视图 | 同上 | 同上 | `pdfview.test.ts`（6 用例）：页码 0/total+1、缩放 0.4/5 | `npx vitest run src/pdfview.test.ts -t "clamps pages into"` | 边界/越界：页码与缩放钳制 | passed | P2 | 通过 | 1 passed |
| FE-21 | MF4 视图 | 同上 | 同上 | `mf4-ui.test.ts`（7 用例）：时间窗 `from=9,to=2`、无时间轴文档 | `npx vitest run src/mf4-ui.test.ts -t "rejects an inverted window"` | 异常：反向时间窗被拒 | passed | P1 | 通过 | 1 passed |
| FE-22 | Git 面板 UI | 同上 | 同上 | `gitpanel.test.ts`（14 用例）：分支名 `bad~name`、远端名 `bad name`、URL `ftp://host/x` | `npx vitest run src/gitpanel.test.ts -t "validates branch names"` | 异常：非法分支/远端/URL 被拒 | passed | P1 | 通过 | 1 passed |
| FE-23 | 崩溃恢复调度 | 同上 | 同上 | `recovery.test.ts`（5 用例）：`Debouncer(2000ms)` 连发 3 次 | `npx vitest run src/recovery.test.ts -t "coalesces repeated schedules"` | 边界：2s 节流合并为一次写入 | passed | P1 | 通过 | 1 passed |
| FE-24 | 界面外壳 | 同上 | 同上 | `shell.test.ts`（7 用例）：宽度 159/160/480/481/NaN | `npx vitest run src/shell.test.ts -t "clamps below min and above max"` | 边界/越界：侧栏宽度 160–480 钳制 | passed | P2 | 通过 | 1 passed |
| FE-25 | 脚本管理 UI | 同上 | 同上 | `script-ui.test.ts`（4 用例）：脚本名 65 字符、`a.txt`、含 `/` 与空格 | `npx vitest run src/script-ui.test.ts -t "validates names"` | 异常/边界：脚本名校验 | passed | P1 | 通过 | 1 passed |
| FE-26 | 其余前端模块 | 同上 | 同上 | `contextmenu`(5)、`encoding-menu`(7)、`promptdialog`(7)、`statusbar`(8)、`tabs`(4)、`macrokeys`(5)、`palette`(8)、`parserpanel`(5)、`mf4-chart`(6)、`mf4-chart.dom`(5)、`mf4-table`(3)、`docKind`(3)、`menu-contract`(5)、`ipc`(26)、`editor`(20)、`mf4-ui` 剩余用例 | `npx vitest run`（全量 39 文件 314 用例）+ 逐条抽查 `contextmenu/encoding-menu/promptdialog/statusbar/tabs` 各 1 条 | 正常/异常：右键菜单不出屏、扩展名大小写、弹窗回车提交、监听降级提示、外部修改黄点 | 全部 passed | P1 | 通过 | 全量 `Test Files 39 passed (39)` / `Tests 314 passed (314)`；抽查逐条：`contextmenu` 1 passed、`encoding-menu` 1 passed、`promptdialog` 1 passed、`statusbar` 1 passed、`tabs` 1 passed |

### 1.7 构建与集成（SYS）

| 编号 | 模块 | 前置条件 | 测试环境 | 测试数据 | 测试步骤 | 测试要点 | 预期结果 | 用例级别 | 测试结果 | 实际结果 |
|---|---|---|---|---|---|---|---|---|---|---|
| SYS-01 | 构建与集成 | 依赖已安装 | cargo 1.99 · macOS | workspace 三包全部测试 | `cargo test -p lumelog-core -p lumelog-can`；`cargo test -p lumelog-app` | 全量自动化回归 | 全部通过 | P1 | 通过 | core 123 项 / can 25 项 / app 184 项，**0 failed**（含 1 条 ignored：`open_1gb_wall_time`） |
| SYS-02 | 构建与集成 | 同上 | Node 24 · macOS | 前端 TS 源码 | `npx tsc --noEmit` | 类型检查 | 无错误、退出码 0 | P1 | 通过 | 无输出，退出码 0 |
| SYS-03 | 构建与集成 | 同上 | Node 24 · macOS | 前端 39 个测试文件 | `npx vitest run --reporter=json` | 前端全量回归 | 0 失败 | P1 | 通过 | `numTotalTests 314 / passed 314 / failed 0`，39 个文件全部 status=passed |
| SYS-04 | 构建与集成 | 同上 | Node 24 · macOS | `app/frontend` 源码 | `npm run build`（tsc + vite build） | 生产构建 | 构建成功 | P1 | 通过 | `✓ built in 5.12s`；仅有 chunk >500kB 的告警，无错误 |
| SYS-05 | 权限·Tauri ACL | 可读配置 | 静态核对 · macOS | `app/src-tauri/capabilities/default.json` | 读文件并核对 `permissions` 数组 | 权限最小化：不得开放 fs/shell/http | 仅 core/dialog 与窗口权限，无 fs/shell/http | P1 | 通过 | permissions 共 9 项（`core:default`、`dialog:default` + 7 项 `core:window:allow-*`），无 `fs:`/`shell:`/`http:` |
| SYS-06 | 集成·启动与关联 | 可读配置 | 静态核对 · macOS | `app/src-tauri/tauri.conf.json` | 核对窗口尺寸、devUrl、fileAssociations | 窗口 1100×720（最小 640×400）、dev 端口 1520、`.log` 关联存在 | 全部匹配 | P2 | 通过 | `width:1100 height:720 minWidth:640 minHeight:400`、`devUrl http://localhost:1520`、`fileAssociations ext:["log"]` |

### 1.8 GUI 交互（GUI）— 无可交互桌面会话

| 编号 | 模块 | 前置条件 | 测试环境 | 测试数据 | 测试步骤 | 测试要点 | 预期结果 | 用例级别 | 测试结果 | 实际结果 |
|---|---|---|---|---|---|---|---|---|---|---|
| GUI-01 | GUI·文档与编码 | 需要 Tauri 桌面窗口 + 交互 | GUI · macOS 桌面会话 | `gbk.log` | 1. `tauri dev` 启动<br>2. 打开 gbk.log<br>3. 看状态栏编码 | 状态栏显示 GBK，切换 UTF-8 再切回不乱码 | 显示 GBK 且可往返切换 | P1 | 未测试 | 未测试原因：当前会话无可交互 GUI 桌面环境，无法拉起 Tauri 窗口并操作状态栏/编码下拉；引擎层等价行为已由 DOC-02/DOC-03 覆盖 |
| GUI-02 | GUI·大文件只读 | 需要 242MB+ 日志 + 交互 | GUI · macOS 桌面会话 | `bench_64mb.log`（本机仅生成 60.7MB） | 1. 生成 >200MiB 日志<br>2. 打开后看状态栏<br>3. 拖动滚动条 | >200MiB 自动只读虚拟列表，行号正确 | 状态栏 `只读 · 共 N 行`，滚动无空白 | P1 | 未测试 | 未测试原因：无可交互 GUI 会话；且本机磁盘余量 15GiB 未生成 >200MiB 样本（磁盘空间与时间限制）。引擎层大文件路径已由 SRC-21 覆盖 |
| GUI-03 | GUI·跳转与选择 | 需要交互 | GUI · macOS 桌面会话 | `utf8.log`（3 行） | 1. 打开文件<br>2. `Ctrl+G` 输入 1 / 0 / 9999<br>3. Alt+拖拽矩形选择 | 0 给出「请输入有效行号（≥1）」，合法行号跳转，矩形选择生效 | 三项行为正确 | P1 | 未测试 | 未测试原因：无可交互 GUI 会话（键盘/鼠标交互无法模拟）；对应前端纯逻辑已由 FE-07/FE-24 覆盖 |
| GUI-04 | GUI·保存冲突 | 需要交互 + 外部改写 | GUI · macOS 桌面会话 | `save_lf.log` | 1. 编辑器内改动<br>2. 外部进程改同一文件<br>3. `Ctrl+S` | 弹出「重新加载 / 强制覆盖 / 取消」三选 | 三选行为正确 | P1 | 未测试 | 未测试原因：无可交互 GUI 会话；冲突判定逻辑已由 APP-05（`recovery`/`commands::reload_tests`）覆盖 |
| GUI-05 | GUI·文档查看器 | 需要真实样本文件 | GUI · macOS 桌面会话 | 仓库内无真实 PDF/BLF/MF4 样本 | 1. 打开 PDF/BLF/MF4<br>2. 翻页/缩放/波形 | 真实渲染、文本可选、波形正确 | 渲染与交互正确 | P1 | 未测试 | 未测试原因：无可交互 GUI 会话 + 仓库内无真实 PDF/BLF/MF4 二进制样本（相关解析逻辑已由 APP-08 与 CAN-24 覆盖） |
| GUI-06 | 集成·单实例与文件关联 | 需要安装应用 + 桌面会话 | GUI · macOS 桌面会话 | 两个 `lumelog-app` 进程 + 一个 `.log` | 1. 启动应用<br>2. 再带 `.log` 路径启动<br>3. 双击关联文件 | 第二次启动不新建窗口，文件在已有实例打开 | 单实例转发生效 | P2 | 未测试 | 未测试原因：本机未安装 LogLume 应用（无 dmg/已安装 App），且无可交互 GUI 会话；静态配置（fileAssociations）已由 SYS-06 核对通过 |

---

## 2. 未通过用例清单

| 编号 | 模块 | 用例要点 | 实际结果 | 严重程度 | 疑似原因 |
|---|---|---|---|---|---|
| DOC-06 | 文档与编码 | 无 BOM 的 UTF-16LE 文件自动识别 | rc=0 但 `encoding:"ISO-8859-1"`，正文乱码（`UTF16LE ,{NLˆ`），中文全部错误 | 中 | `crates/lumelog-core/src/encoding/mod.rs:131-189` 的 `detect()` 仅通过 BOM（`FF FE`/`FE FF`）判定 UTF-16，chardetng 不支持 UTF-16；无 BOM 时无任何启发式分支落到 `Utf16Le/Be`，回退路径按单字节编码判定 |
| DOC-07 | 文档与编码 | 无 BOM 的 UTF-16BE 文件自动识别 | rc=0 但 `encoding:"GBK"`，正文乱码（`UTF16BE {,N圠`） | 中 | 同 DOC-06：BE 无 BOM 同样无识别分支，被 chardetng 判为 GBK；README 声称「UTF-8 / UTF-16 … 自动识别与切换」，与实现存在差距 |
| DOC-08 | 文档与编码 | 手动切换 UTF-16 LE 后按行取块，行尾不得出现替换符 | 第二行末尾多出 `EF BF BD`（U+FFFD）；同内容带 BOM 的对照文件输出字节干净 | 中 | `crates/lumelog-core/src/doc/file.rs:269-273` 的 `set_encoding()` 只改 label 并清缓存，不按新编码重建行索引；行边界仍按字节偏移（文件 122-140 行的 UTF-16 对齐索引仅在 open 阶段按探测编码建立），行尾落在半个码元上被解码为 U+FFFD |
| DOC-37 | 文档与编码 | 只读文件（chmod 444）保存应被拒绝且内容不变 | rc=0 且 `{"saved":true,...}`，文件内容被改写为 `new content\n`，只读位被绕过 | 中 | `crates/lumelog-core/src/doc/save.rs:207-234` 的 `atomic_write()` 走「临时文件 + rename」覆盖，Unix 下 rename 只要求目录可写、不受目标文件只读位限制；只读特判仅存在于 `#[cfg(windows)]` 分支（save.rs:534-567），macOS/Linux 无对应判定 |
| DOC-48 | 文档与编码 | CLI 应支持 `--help` | rc=2，`unknown option '--help' for subcommand 'open'`；同仓库 `lumelog-can help` 可正常打印用法 | 低 | `crates/lumelog-core/src/bin/lumelog.rs:76-102` 的 `Args::validate()` 白名单里没有 `help`，主分发（124-132 行）也未把 `--help`/`-h` 映射到 USAGE |

（说明：CAN-17 `--limit 0` 在 `--out jsonl` 下被忽略、`--limit` 与 `--out` 互斥关系未文档化，按「文档未承诺」处理，未列入未通过清单，但行为与直觉不一致，见 §3 观察项。）

---

## 3. 观察项（非失败，供改进）

1. **CAN-17**：`lumelog-can decode --out jsonl --limit 0` 仍输出全部帧——`bin/lumelog-can.rs:169-172` 中 `print_all` 优先于 `limit`，`--limit` 仅在未指定 `--out` 时生效；建议明确二者互斥或在帮助中说明。
2. **DOC-14**：GB18030 文件被标注为 `GBK`（`encoding_rs` 的 GBK 解码器兼容 GB18030 四字节，内容正确），状态栏/摘要里的编码名与用户预期可能不符。
3. **SYS-04**：前端生产构建出现 chunk > 500kB 告警（最大产物 684.21 kB，gzip 221.84 kB），可考虑分包。
4. **工作区状态**：`git status` 显示 `docs/reviews/2026-11-10-blackbox-test-cases.md`、`…-report.md` 处于已删除状态，且存在未跟踪目录 `docs/reviews/.notelume-backup/`（非本次测试产生，本次测试未写入仓库任何文件）。
5. **CLI 行号基准**：`lumelog search` 输出的 `line` 为 0 基，与 GUI 侧 1 基行号（`Ctrl+G`）口径不同，跨端脚本引用需注意。

---

## 4. 用例质量门禁与缺陷模式匹配（test-mentor 脚本实测）

| 项目 | 命令 | 结果 |
|---|---|---|
| 用例质量门禁 | `node ~/.codebuddy/skills/test-mentor/scripts/case-gate.js --file /tmp/loglume-tc-20261110/cases.json --md` | 结论 **通过**，评分 **92/100**（阈值 70），风险等级 **低**；五维：功能覆盖 100 / 数据设计 100 / 可执行性 100 / 集成与契约 75 / 非功能 75；负例+边界占比 **73%**；唯一遗留建议为"补上下游联动验证" |
| 缺陷模式匹配 | `node ~/.codebuddy/skills/test-mentor/scripts/defect-kb.js match --file bug.json` | 命中模式 **DP-14 时区与编码问题（数据一致性）**，根因线索"编码未显式声明"与本次 DOC-06/07/08 的 UTF-16 识别缺陷一致；历史相似 Bug：无 |

---

## 5. 复现方式（全部命令均可直接执行）

```bash
cd /Users/cc/Downloads/项目/log-lume

# 1) 全量 Rust 回归
cargo test -p lumelog-core -p lumelog-can
cargo test -p lumelog-app

# 2) 单条精确用例（示例）
cargo test -p lumelog-app ai::stream_tests::http_429_maps_to_rate_limited -- --exact
cargo test -p lumelog-core --test docfile open_rejects_file_over_size_limit -- --exact

# 3) 前端
cd app/frontend
npx tsc --noEmit
npx vitest run                       # 全量 314
npx vitest run src/canfilter.test.ts -t "id_range_inclusive"   # 单条

# 4) CLI 黑盒（夹具脚本在 /tmp/loglume-tc-20261110）
./target/debug/lumelog open --file /tmp/loglume-tc-20261110/utf8.log
./target/debug/lumelog search --file /tmp/loglume-tc-20261110/gbk.log --pattern 错误 --max 10
./target/debug/lumelog-can decode --log crates/lumelog-can/tests/data/candump_basic.log \
    --format candump --dbc crates/lumelog-can/tests/data/sample.dbc --out jsonl
bash /tmp/loglume-tc-20261110/run_cli.sh          # DOC/SRC/CAN 全部命令
bash /tmp/loglume-tc-20261110/run_rust_app.sh     # APP 精确用例
```

> 说明：`target/`、`dist/` 为构建产物（已在 `.gitignore`），`npm run build` 会刷新 `app/frontend/dist`；本次测试未修改仓库任何源码或配置。
