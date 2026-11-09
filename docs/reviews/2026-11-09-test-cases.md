# LumeLog 测试用例与逐条执行报告（2026-11-09）

## 1. 范围

被测对象为 `e:\LogLume` 当前源码构建出的 LumeLog：

- 编码识别与解码：`crates/lumelog-core`（BOM、UTF-8/GBK/UTF-16、GB18030、Big5、Shift-JIS、EUC-JP/KR、KOI8-U、不可映射字符、手动切换）。
- 大文件打开、行索引与搜索：`crates/lumelog-core`（242.8MB 文件索引、按行取块、字面/正则搜索、`--max` 截断、8MB 窗口接缝、取消入口）。
- CAN/DBC 解码：`crates/lumelog-can`（DBC 严格/宽松解析、Intel/Motorola 位序、candump/ASC/自定义正则源、坏帧与非法输入、CAN-FD/RTR/扩展帧）。
- GUI 类功能：编辑器/查询面板/编码下拉/保存冲突、行过滤与高亮、时序筛选、主题、崩溃恢复、会话恢复、快捷键、CAN 面板过滤、文件监听等。

## 2. 测试环境与构建说明

| 项 | 值 |
|---|---|
| 操作系统 | Windows 11 家庭版 中文版 10.0.26300（64 位） |
| 终端 | PowerShell 5.1；`python` 3.x 用于生成二进制夹具与采集退出码/耗时 |
| 被测二进制 | `target\release\lumelog.exe` SHA256 `A758F71B…88D16C7C`；`target\release\lumelog-can.exe` SHA256 `98993DA3…C1E605E`；`target\release\lumelog-gen.exe` SHA256 `9925AA36…96AF9F4` |
| 夹具目录 | `%TEMP%\lumelog-tests-20261109`（仓库外，未向仓库写入任何测试文件） |
| 过滤引擎夹具 | `%TEMP%\lumelog-filter-harness`，`Cargo.toml` 仅依赖 `lumelog-core` 的 path，`CARGO_TARGET_DIR=%TEMP%\lumelog-harness-target` |
| 前端纯函数 | `app/frontend` 下既有 Vitest 套件（`npx vitest run …`，无头执行） |

构建说明：仓库内原 `target\release\lumelog.exe` 时间戳为 2026-10-24，早于源码中的编码检测改动（实测旧二进制把 EUC-JP/KR 识别为 GBK）。为保证“测的是当前源码”，本次执行前运行了：

```powershell
cargo build --release -p lumelog-core -p lumelog-can --bin lumelog --bin lumelog-gen --bin lumelog-can
```

仅重新编译，未修改任何仓库代码。`lumelog-gen.exe 256` 生成 `bench_256mb.log`（242,808,915 字节、4,194,305 行），`lumelog-gen.exe 8` 生成 `bench_8mb.log`（7,587,772 字节）。

### 2.1 主要夹具（真实字节/真实内容）

| 夹具 | 内容 |
|---|---|
| `utf8.log` | `台架 HIL 测试 OK\n温度=25.0\n`，33 字节，UTF-8 |
| `gbk.log` | `动力系统控制台台架测试\n错误 记录: 传感器异常\n`，GBK 编码，45 字节 |
| `utf8bom.log` | `EF BB BF` + `BOM 文件第一行\n第二行\n` |
| `utf16le.log` | `FF FE` + `第一行 UTF-16LE\n第二行\n`（LE），36 字节 |
| `utf16be.log` | `FE FF` + `第一行 UTF-16BE\n第二行\n`（BE），36 字节 |
| `gb18030_4byte.log` | `扩展字符 𠀀 测试\n`（U+20000，GB18030 四字节序列），19 字节 |
| `big5_2.log` | `繁體中文測試日誌 錯誤 溫度感測器異常 2026-10-30\n` ×20，Big5，1,140 字节 |
| `sjis2.log` | `システム起動ログ 温度センサー値を記録しました 2026-10-30\n` ×20，Shift-JIS |
| `eucjp2.log` / `euckr2.log` | 日文/韩文各 20 行，EUC-JP 1,160 字节 / EUC-KR 1,080 字节 |
| `koi8r.log` | 俄文 `Запуск системы: …` ×20，KOI8-R 编码，1,120 字节 |
| `invalid_gbk.log` | 字节 `CA 0A 78 0A`（悬挂 GBK 前导字节 `CA`） |
| `interior_bom.log` | `first\n` + `EF BB BF` + `second\n`（第二行行首内嵌 BOM 字节） |
| `tail.log` | 199 行 `val=000..997` + 末行 `2026-11-09 10:03:19 INFO val=999 tail`，6,605 字节 |
| `longline.log` | 10,485,760 个 `A` + `\nneedle\n`，10,485,768 字节 |
| `seam.log` | 9MiB+ 单行，`SEAMNEEDLE` 起始字节 8,388,605（跨 8MiB 搜索窗口接缝），9,437,190 字节 |
| `sample.dbc` / `bitorder.dbc` | 仓库 `crates/lumelog-can/tests/data` 复制（含 1 行坏 `SG_`，及 Intel/Motorola 位序信号） |
| `candump_basic.log` / `cli_intel.log` / `asc_basic.asc` | 仓库既有 CAN 夹具复制 |
| `bad_candump2.log` | 奇数 hex、非 hex、非法 ID、65 字节 payload、空行、1 条合法帧 |
| `p65.log` | `can0 123#` + `AB`×65（65 字节，超 64 上限） |
| `emoji_payload.log` | `can0 123#😀`（非 ASCII payload） |
| `rtr_ext_fd.log` | `can0 123#R`、`can0 1FFFFFFF#42`、`can0 123##03112233…` |
| `bad.dbc` | 坏 `BO_`、孤立 `SG_`、坏 `SG_`、1 条合法消息 |
| `custom_regex2.log` | `T=12.345 ID=0x1A2 DATA=11:22:33:44`、`T=13.700 ID=0x7FF DATA=AA-BB` |
| `asc_bad.asc` | 1 条合法 ASC 帧 + `garbage line here` + 非法 ID `ZZZ` |

## 3. 统计

| 模块 | 用例数 | 通过 | 不通过 | 未测试 |
|---|---:|---:|---:|---:|
| 编码识别与解码 | 25 | 22 | 3 | 0 |
| 大文件、行索引与搜索 | 22 | 18 | 2 | 2 |
| CAN/DBC 解码 | 25 | 23 | 1 | 1 |
| GUI 类功能 | 20 | 11 | 0 | 9 |
| **合计** | **92** | **74** | **6** | **12** |

- 已执行用例 = 92 − 12 = 80，已执行通过率 = 74 ÷ 80 = **92.5%**；按全部用例计通过率 = 74 ÷ 92 = **80.4%**。
- 异常 + 边界用例共 **49** 条，占比 **49 ÷ 92 × 100% = 53.26%**（≥ 40% 要求）。
- 级别分布：P0 17 条、P1 41 条、P2 34 条。

异常/边界用例编号（49 条）：

- 编码：E06、E14、E15、E16、E17、E18、E19、E20、E21、E22、E23（11）
- 大文件/搜索：B01、B07、B08、B09、B10、B11、B13、B14、B15、B17、B18、B20、B21、B22（14）
- CAN/DBC：C01、C03、C04、C05、C11、C13、C14、C15、C16、C17、C18、C19、C20、C21、C22、C24（16）
- GUI：G02、G04、G05、G09、G10、G11、G15、G16（8）

## 4. 用例明细

> 路径中的 `<T>` 代表 `%TEMP%\lumelog-tests-20261109`；“rc”为进程退出码；“t”为 Python 采集的墙钟耗时（含进程启动）。

### 4.1 编码识别与解码（25 条）

| 编号 | 模块 | 前置条件 | 测试环境 | 测试数据 | 测试步骤 | 测试要点 | 预期结果 | 用例级别(P0/P1/P2) | 测试结果 | 实际结果 |
|---|---|---|---|---|---|---|---|---|---|---|
| E01 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | `utf8.log`（33B，UTF-8 中文+ASCII） | `lumelog open --file <T>\utf8.log` | UTF-8 自动检测与行数 | 返回 `encoding:"UTF-8"`、`lines:3`、`size:33`、`eol:"lf"` | P0 | 通过 | rc=0，t=0.01s，输出 `{"size":33,"lines":3,"encoding":"UTF-8","eol":"lf"}` |
| E02 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | `gbk.log`（45B，GBK） | `lumelog open --file <T>\gbk.log` | GBK 自动检测 | 返回 `encoding:"GBK"`、`lines:3`、`size:45` | P0 | 通过 | rc=0，`{"size":45,"lines":3,"encoding":"GBK","eol":"lf"}` |
| E03 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | `utf8bom.log`（`EF BB BF` 开头） | `lumelog open --file <T>\utf8bom.log --head 1` | BOM 识别与剥除 | `encoding:"UTF-8 BOM"`；首行输出 `BOM 文件第一行`，不含 BOM 字节 | P1 | 通过 | rc=0，`{"size":33,"lines":3,"encoding":"UTF-8 BOM","eol":"lf"}`；首行文本 `BOM 文件第一行` |
| E04 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | `utf16le.log`（BOM+LE，36B） | `lumelog open --file <T>\utf16le.log --head 2` | UTF-16LE 识别与解码 | `encoding:"UTF-16 LE"`；两行中文正确 | P0 | 通过 | rc=0，`encoding:"UTF-16 LE"`、`lines:3`；输出 `第一行 UTF-16LE`、`第二行` |
| E05 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | `utf16be.log`（BOM+BE，36B） | `lumelog open --file <T>\utf16be.log` | UTF-16BE 识别 | `encoding:"UTF-16 BE"`、`lines:3` | P1 | 通过 | rc=0，`{"size":36,"lines":3,"encoding":"UTF-16 BE","eol":"lf"}` |
| E06 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | `gb18030_4byte.log`（U+20000，GB18030 四字节） | `lumelog open --file <T>\gb18030_4byte.log --head 1` | GB18030 四字节边界字符 | 标签为 GBK/GB18030 均可，但首行必须含 `𠀀` 而非 U+FFFD | P2 | 通过 | 标签 `GBK`，首行 `扩展字符 𠀀 测试`；GBK 解码器正确处理四字节序列 |
| E07 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | `big5_2.log`（Big5，1,140B） | `lumelog open --file <T>\big5_2.log --head 2`（不传 `--encoding`） | Big5 自动识别正确性 | 应识别为 `BIG5`，首行为 `繁體中文測試日誌…` | P1 | 不通过 | 识别为 `GBK`，首行乱码 `羉砰いゅ代刚ら粁 岿粇 放…稰代竟钵盽 2026-10-30`；手动 `--encoding BIG5` 后正常（见 E08） |
| E08 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | `big5_2.log` | `lumelog open --file <T>\big5_2.log --encoding BIG5 --head 2` | 手动切换到 Big5 | `encoding:"BIG5"`，首行 `繁體中文測試日誌 錯誤 溫度感測器異常 2026-10-30` | P1 | 通过 | rc=0，`{"encoding":"BIG5","lines":21}`；首行中文正确 |
| E09 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | `sjis2.log`（Shift-JIS，1,140B） | `lumelog open --file <T>\sjis2.log --head 2` | Shift-JIS 自动识别正确性 | 应识别为 `Shift-JIS`，首行为日文原文 | P1 | 不通过 | 识别为 `GBK`，首行乱码 `僔僗僥儉婲摦儘僌 壏搙僙儞僒乕抣傪婰榐偟傑偟偨 2026-10-30`；手动 `--encoding Shift-JIS` 后正常（见 E10） |
| E10 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | `sjis2.log` | `lumelog open --file <T>\sjis2.log --encoding Shift-JIS --head 1` | 手动切换到 Shift-JIS | `encoding:"Shift-JIS"`，首行 `システム起動ログ 温度センサー値を記録しました 2026-10-30` | P2 | 通过 | rc=0，`{"size":1140,"lines":21,"encoding":"Shift-JIS"}`；日文正确 |
| E11 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | `eucjp2.log`（EUC-JP，1,160B） | `lumelog open --file <T>\eucjp2.log --head 1` | EUC-JP 自动识别 | `encoding:"EUC-JP"`，日文正确 | P1 | 通过 | rc=0，`{"size":1160,"lines":21,"encoding":"EUC-JP"}`；首行日文正确 |
| E12 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | `euckr2.log`（EUC-KR，1,080B） | `lumelog open --file <T>\euckr2.log --head 1` | EUC-KR 自动识别 | `encoding:"EUC-KR"`，韩文正确 | P1 | 通过 | rc=0，`{"size":1080,"lines":21,"encoding":"EUC-KR"}`；首行韩文正确 |
| E13 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | `koi8r.log`（俄文，1,120B） | `lumelog open --file <T>\koi8r.log --head 1` | KOI8-R 卢布/西里尔识别 | `encoding:"KOI8-U"` 或 `KOI8-R`，俄文正确 | P2 | 通过 | `encoding:"KOI8-U"`，首行 `Запуск системы: журнал тестирования датчиков 2026-11-09` |
| E14 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | `utf16le.log` | `lumelog search --file <T>\utf16le.log --pattern 第一行` | UTF-16 搜索拒绝（异常） | rc=1，错误含 “UTF-16 search unsupported” | P1 | 通过 | rc=1，stderr `lumelog: encoding error: UTF-16 search unsupported in v1` |
| E15 | 编码识别与解码 | 夹具已生成，种子为 GBK | Win11 · lumelog.exe release | `unmap_seed.log`（内容 `原始内容\n` 的 GBK 字节）；stdin 输入 `ok😀\n` | 记录文件 SHA256 → `lumelog save --file <T>\unmap_seed.log --encoding GBK --stdin` → 再次计算 SHA256 | 不可映射字符保存（异常） | rc=1，提示不可映射；文件字节不变 | P1 | 通过 | rc=1，stderr 原文 `lumelog: 1 characters are unmappable in the target encoding`；SHA256 前后一致 |
| E16 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | `empty.log`（0 字节） | `lumelog open --file <T>\empty.log` | 空文件边界 | `size:0`、`lines:1`、`encoding:"UTF-8"`、`eol:"lf"` | P2 | 通过 | rc=0，`{"size":0,"lines":1,"encoding":"UTF-8","eol":"lf"}` |
| E17 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | `invalid_gbk.log`（`CA 0A 78 0A`） | `lumelog open --file <T>\invalid_gbk.log --head 2` | 非法字节/不可映射（异常） | 检测不崩溃（GBK/GB18030），首行出现 U+FFFD 替换符，第二行 `x` | P2 | 通过 | `encoding:"GB18030"`；首行 `�`（U+FFFD），第二行 `x`；rc=0 |
| E18 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | `<T>` 目录本身 | `lumelog open --file <T>` | 目录当文件打开（异常） | rc=1，报 IO/拒绝访问 | P2 | 通过 | rc=1，`lumelog: io error: 拒绝访问。 (os error 5)` |
| E19 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | 不存在的 `nope.log` | `lumelog open --file <T>\nope.log` | 文件不存在（异常） | rc=1，报找不到文件 | P2 | 通过 | rc=1，`lumelog: io error: 系统找不到指定的文件。 (os error 2)` |
| E20 | 编码识别与解码 | 夹具已生成，ACL 已拒绝当前用户读取 | Win11 · lumelog.exe release | `acl.log`，执行 `icacls acl.log /inheritance:r /deny "%USERNAME%:(R)"` | 打开 `acl.log`；随后 `icacls /reset` 恢复 | 权限拒绝（异常） | rc=1，报拒绝访问 | P1 | 通过 | rc=1，`lumelog: io error: 拒绝访问。 (os error 5)`；恢复后读取正常 |
| E21 | 编码识别与解码 | 网络路径不可达 | Win11 · lumelog.exe release | `\\no-such-host-lumelog\share\x.log` | `lumelog open --file \\no-such-host-lumelog\share\x.log` | 网络异常（异常） | rc=1，快速失败并报网络路径错误 | P2 | 通过 | rc=1，t=1.33s，`lumelog: io error: 找不到网络路径。 (os error 53)` |
| E22 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | `interior_bom.log`（第二行行首 `EF BB BF`） | `lumelog open --file <T>\interior_bom.log --head 2` | 内嵌 BOM 不作文件头剥除（边界） | 第二行文本以 U+FEFF 开头，内容 `second` | P2 | 通过 | 第二行 repr 为 `'\ufeffsecond'`，U+FEFF 保留 |
| E23 | 编码识别与解码 | 夹具已生成，文件属性 ReadOnly | Win11 · lumelog.exe release | `ro2.log`（内容 `ro\n`，属性 ReadOnly） | 记录 SHA256 → `lumelog save --file <T>\ro2.log --stdin`（输入 `new\n`） | 只读文件保存（异常） | rc≠0；提示权限/只读；文件不变 | P2 | 不通过 | rc=1，但错误为 `lumelog: target file is locked by another process`（误导为“被占用”）；SHA256 前后一致 |
| E24 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | `save_lf.log`（原 `old\n`）；stdin `new1\nnew2\n` | `lumelog save --file <T>\save_lf.log --stdin` | 保存默认 LF | rc=0，`saved:true`；落盘字节 `new1\nnew2\n` | P0 | 通过 | rc=0，`{"saved":true,"replace_count":0,"conflict":false}`；字节 `b'new1\nnew2\n'` |
| E25 | 编码识别与解码 | 夹具已生成 | Win11 · lumelog.exe release | `save_crlf2.log`（原 `old\r\n`）；stdin `a\nb\n` | `lumelog save --file <T>\save_crlf2.log --eol crlf --stdin` | 保存 CRLF | rc=0；落盘字节 `a\r\nb\r\n` | P1 | 通过 | rc=0，`{"saved":true,"replace_count":0,"conflict":false}`；字节 `b'a\r\nb\r\n'` |

### 4.2 大文件、行索引与搜索（22 条）

| 编号 | 模块 | 前置条件 | 测试环境 | 测试数据 | 测试步骤 | 测试要点 | 预期结果 | 用例级别(P0/P1/P2) | 测试结果 | 实际结果 |
|---|---|---|---|---|---|---|---|---|---|---|
| B01 | 大文件、行索引与搜索 | 已用 lumelog-gen 生成 | Win11 · lumelog.exe release | `bench_256mb.log`（242,808,915B，4,194,305 行） | `lumelog open --file <T>\bench_256mb.log --line-count` | 大文件全量行索引（边界） | 行数精确 4,194,305；索引在秒级完成 | P0 | 通过 | rc=0，t=0.16s，输出 `4194305` |
| B02 | 大文件、行索引与搜索 | 同上 | Win11 · lumelog.exe release | `bench_256mb.log` | `lumelog open --file <T>\bench_256mb.log --head 100` | 首屏 100 行按需取块 | 返回 100 行，内容正确，耗时不随全文增大 | P0 | 通过 | rc=0，t=0.17s；打印 100 行，末行 `2026-10-24 12:00:38.900 INFO  [task-2] ECUsync ok val=900` |
| B03 | 大文件、行索引与搜索 | 同上 | Win11 · lumelog.exe release | `bench_256mb.log`；pattern `val=999` | `lumelog search --file <T>\bench_256mb.log --pattern val=999 --max 50` | 字面全量搜索 + 截断 | `matches` 统计完整、`truncated:true`、最多返回 50 条 | P0 | 通过 | rc=0，t=0.29s，`{"matches":4151,"scanned_bytes":242808915,"truncated":true}`；首命中 `line:919, byte_in_line:50, len:7` |
| B04 | 大文件、行索引与搜索 | 同上 | Win11 · lumelog.exe release | `bench_8mb.log`（7,587,772B）；pattern `val=9[0-9][0-9]` | `lumelog search --file <T>\bench_8mb.log --pattern "val=9[0-9][0-9]" --regex --max 20` | 正则搜索 | 正确统计命中、截断返回 | P1 | 通过 | rc=0，t=0.14–0.51s，`{"matches":13168,"scanned_bytes":7587772,"truncated":true}` |
| B05 | 大文件、行索引与搜索 | 同上 | Win11 · lumelog.exe release | `bench_8mb.log`；pattern `info` | `lumelog search --file <T>\bench_8mb.log --pattern info --ignore-case --max 20` | 忽略大小写大量命中性能 | 命中数正确（131,072），8MB 文件应在 10s 内完成（参照 P5 1GB≤10s 门槛） | P1 | 不通过 | rc=0，`{"matches":131072,"scanned_bytes":7587772,"truncated":true}`，但 t=48.98s；仅返回 20 条仍耗时近 49 秒 |
| B06 | 大文件、行索引与搜索 | 夹具已生成 | Win11 · lumelog.exe release | `tail.log`（200 条含 INFO + 末尾空行）；`--max 5` | `lumelog search --file <T>\tail.log --pattern INFO --max 5` | `--max` 截断 | 返回 5 条，`matches:200`、`truncated:true` | P1 | 通过 | rc=0，t=0.01s，5 条 `line 0..4`，摘要 `{"matches":200,"scanned_bytes":6605,"truncated":true}` |
| B07 | 大文件、行索引与搜索 | 夹具已生成 | Win11 · lumelog.exe release | `tail.log`；`--max 0` | `lumelog search --file <T>\tail.log --pattern INFO --max 0` | `--max=0` 下边界 | 不返回命中行，但仍完成统计并标记截断 | P2 | 通过 | rc=0，0 条命中；`{"matches":200,"scanned_bytes":6605,"truncated":true}` |
| B08 | 大文件、行索引与搜索 | 夹具已生成 | Win11 · lumelog.exe release | `tail.log`；空 pattern | `lumelog search --file <T>\tail.log --pattern ""` | 空搜索串（异常） | rc=1，报空搜索模式 | P1 | 通过 | rc=1，`lumelog: invalid search pattern: empty search pattern` |
| B09 | 大文件、行索引与搜索 | 夹具已生成 | Win11 · lumelog.exe release | `tail.log`；pattern `(` | `lumelog search --file <T>\tail.log --pattern "(" --regex` | 非法正则（异常） | rc=1，报 regex parse error | P1 | 通过 | rc=1，`lumelog: invalid search pattern: regex parse error: … error: unclosed group` |
| B10 | 大文件、行索引与搜索 | 夹具已生成 | Win11 · lumelog.exe release | `gbk.log`；pattern `😀` | `lumelog search --file <T>\gbk.log --pattern 😀` | 目标编码不可表示（异常） | rc=1，提示 pattern 不可表示 | P2 | 通过 | rc=1，`lumelog: invalid search pattern: pattern is not representable in the file encoding` |
| B11 | 大文件、行索引与搜索 | 夹具已生成 | Win11 · lumelog.exe release | 不存在的 `nope.log` | `lumelog search --file <T>\nope.log --pattern x` | 文件不存在（异常） | rc=1，报找不到文件 | P2 | 通过 | rc=1，`lumelog: io error: 系统找不到指定的文件。 (os error 2)` |
| B12 | 大文件、行索引与搜索 | 夹具已生成 | Win11 · lumelog.exe release | `gbk.log`；pattern `错误`（GBK 中文） | `lumelog search --file <T>\gbk.log --pattern 错误 --max 10` | GBK 中文搜索与字节偏移 | 命中第 1 行，`byte_in_line:0`、`len:4`，片段含中文 | P0 | 通过 | rc=0，t=0.01s，`{"line":1,"byte_in_line":0,"len":4,"snippet":"错误 记录: 传感器异常"}`，`matches:1` |
| B13 | 大文件、行索引与搜索 | 夹具已生成 | Win11 · lumelog.exe release | `tail.log`；pattern `val=999` | `lumelog search --file <T>\tail.log --pattern val=999 --max 10` | 末行命中边界 | 命中 `line:199`、`byte_in_line:25`、`matches:1` | P1 | 通过 | rc=0，`{"line":199,"byte_in_line":25,"len":7,"snippet":"2026-11-09 10:03:19 INFO val=999 tail"}`，`matches:1` |
| B14 | 大文件、行索引与搜索 | 夹具已生成 | Win11 · lumelog.exe release | `empty.log`；pattern `x` | `lumelog search --file <T>\empty.log --pattern x` | 空文件搜索（边界） | `matches:0`、`scanned_bytes:0`，不报错 | P1 | 通过 | rc=0，`{"matches":0,"scanned_bytes":0,"cancelled":false,"truncated":false}` |
| B15 | 大文件、行索引与搜索 | 夹具已生成 | Win11 · lumelog.exe release | `tail.log`（201 行） | `lumelog open --file <T>\tail.log --head 1000000` | 请求行数超总行数（边界） | 打印全部 201 行，不报错、不越界 | P2 | 通过 | rc=0，t=0.02s；JSON `lines:201`，打印 201 行（含末尾空行） |
| B16 | 大文件、行索引与搜索 | 夹具已生成 | Win11 · lumelog.exe release | `crlf.log`（`line1\r\nline2\r\n`） | `lumelog open --file <T>\crlf.log --line-count` 与 `--head 2` | CRLF 行尾识别 | `eol:"crlf"`、`lines:3`；首两行 `line1`、`line2` | P1 | 通过 | rc=0，`{"size":14,"lines":3,"encoding":"UTF-8","eol":"crlf"}` |
| B17 | 大文件、行索引与搜索 | 夹具已生成 | Win11 · lumelog.exe release | `no_trailing_nl.log`（`a\nb`） | `lumelog open --file <T>\no_trailing_nl.log --line-count` | 无尾换行行数（边界） | `lines:2` | P2 | 通过 | rc=0，输出 `2` |
| B18 | 大文件、行索引与搜索 | 夹具已生成 | Win11 · lumelog.exe release | `longline.log`（单行 10MiB + `needle`） | `lumelog open --line-count`；`lumelog search --pattern needle` | 超长单行（边界） | `lines:3`；搜索命中 `line:1`、`byte_in_line:0`、`len:6`，不 OOM | P2 | 通过 | open `{"size":10485768,"lines":3}`；search t=0.03s，`{"line":1,"byte_in_line":0,"len":6,"snippet":"needle"}` |
| B19 | 大文件、行索引与搜索 | 需 GUI/IPC | Win11 · 无 GUI 会话 | `bench_256mb.log`，搜索 `val=999` 后 0.5s 内点“取消” | 通过 GUI 查询面板触发 `search_run`，再触发 `search_cancel` | 搜索取消 | `cancelled:true`，已扫描字节小于全文件 | P1 | 未测试 | CLI `search` 无取消参数；取消由 `search_stream` 的 progress 回调与 IPC `search_cancel` 驱动，本次无 Tauri GUI 会话，无法注入点击 |
| B20 | 大文件、行索引与搜索 | 需 GUI/IPC | Win11 · 无 GUI 会话 | `bench_256mb.log`（242.8MB > 200MiB） | 通过 GUI 打开该文件，观察状态栏模式与滚动 | >200MB 只读虚拟列表（边界） | `doc_open` 返回 `readonly:true`，不建立全量行索引，滚动按需取块 | P1 | 未测试 | 只读阈值在 `app/src-tauri/src/commands.rs` 的 `doc_open`（`size > 200MiB`）中；CLI `open` 始终全量索引，无只读/虚拟列表入口，且无 GUI 会话可观察 |
| B21 | 大文件、行索引与搜索 | 夹具已生成 | Win11 · lumelog.exe release | `seam.log`（9,437,190B）；`SEAMNEEDLE` 起始字节 8,388,605（跨 8MiB 窗口接缝） | `lumelog search --file <T>\seam.log --pattern SEAMNEEDLE --max 5` | 跨窗口接缝不重不漏（边界） | 恰好命中 1 次，`byte_in_line:8388605`，`len:10` | P1 | 通过 | rc=0，t=0.06s，`{"line":0,"byte_in_line":8388605,"len":10}`，`matches:1`、`scanned_bytes:9437190` |
| B22 | 大文件、行索引与搜索 | 夹具已生成 | Win11 · lumelog.exe release | `tail.log`；未知选项 `--mode fuzzy` | `lumelog search --file <T>\tail.log --pattern x --mode fuzzy` | 未知选项应被拒绝（异常） | rc≠0，提示未知选项（CLI 实际只支持 `--regex`，无 `--mode`） | P1 | 不通过 | rc=0，未知选项被静默忽略并返回 `{"matches":0,"scanned_bytes":6605}` |

### 4.3 CAN/DBC 解码（25 条）

| 编号 | 模块 | 前置条件 | 测试环境 | 测试数据 | 测试步骤 | 测试要点 | 预期结果 | 用例级别(P0/P1/P2) | 测试结果 | 实际结果 |
|---|---|---|---|---|---|---|---|---|---|---|
| C01 | CAN/DBC 解码 | 夹具已复制 | Win11 · lumelog-can.exe release | `sample.dbc`（第 14 行 `SG_ BrokenSignal : 24\|notanint@1+ …`） | `lumelog-can dbc --file <T>\sample.dbc` | DBC 严格模式遇坏行（异常） | rc=1，指出第 14 行语法错误 | P0 | 通过 | rc=1，stderr `dbc: dbc syntax error at line 14 (bad SG_): SG_ BrokenSignal : 24\|notanint@1+ ...` |
| C02 | CAN/DBC 解码 | 同上 | Win11 · lumelog-can.exe release | `sample.dbc` | `lumelog-can dbc --file <T>\sample.dbc --lenient` | 宽松模式跳过坏行 | `messages:3`、`signals:6`、`errors:1` | P0 | 通过 | rc=0，`{"messages":3,"signals":6,"errors":1}` |
| C03 | CAN/DBC 解码 | 夹具已生成 | Win11 · lumelog-can.exe release | `empty.canlog`（0 字节 DBC） | `lumelog-can dbc --file <T>\empty.canlog` | 空 DBC（边界） | `messages:0`、`signals:0`、`errors:0` | P2 | 通过 | rc=0，`{"messages":0,"signals":0,"errors":0}` |
| C04 | CAN/DBC 解码 | 夹具已生成 | Win11 · lumelog-can.exe release | 不存在的 `no.dbc` | `lumelog-can dbc --file <T>\no.dbc` | DBC 不存在（异常） | rc=1，报无法读取 | P2 | 通过 | rc=1，`dbc: cannot read C:\Users\86153\AppData\Local\Temp\lumelog-tests-20261109\no.dbc` |
| C05 | CAN/DBC 解码 | 夹具已生成 | Win11 · lumelog-can.exe release | `bad.dbc`（坏 BO_ + 孤立 SG_ + 坏 SG_ + 1 条合法消息） | `lumelog-can dbc --file <T>\bad.dbc --lenient` | 多类非法行（异常） | `messages:1`、`signals:1`、`errors:3` | P1 | 通过 | rc=0，`{"messages":1,"signals":1,"errors":3}` |
| C06 | CAN/DBC 解码 | 夹具已复制 | Win11 · lumelog-can.exe release | `candump_basic.log` + `sample.dbc` | `lumelog-can decode --log <T>\candump_basic.log --format candump --dbc <T>\sample.dbc` | candump 解析 + Intel 解码 | `decoded:7`、`skipped:2`；`0x123` 帧 EngineSpeed raw 44510/physical 5563.75 | P0 | 通过 | rc=0，`{"decoded":7,"skipped":2}`；首帧 `EngineSpeed raw:44510 physical:5563.75 rpm`、`CoolantTemp raw:-66 physical:-106 degC` |
| C07 | CAN/DBC 解码 | 同上 | Win11 · lumelog-can.exe release | 同上 | 加 `--out jsonl` | 全量 JSONL 输出 | 7 帧全部输出，含 RTR（dlc0）、扩展 ID `0x1FFFFFFF`、通道 2 帧 | P0 | 通过 | 7 条帧 JSON：`0x123/DEADBEEF`、`0x735`、`0x1F dlc9`、`0x123 dlc0`、`0x123/1122334455667788`、`0x1FFFFFFF/42`、`0x7AA/0011 (ts 0.0005)`；统计同一行 |
| C08 | CAN/DBC 解码 | 夹具已复制 | Win11 · lumelog-can.exe release | `cli_intel.log`（`can0 64#DEADBEEF12345678`）+ `bitorder.dbc` | `lumelog-can decode --log <T>\cli_intel.log --format candump --dbc <T>\bitorder.dbc --out jsonl` | Intel 位序信号 | `I_U8=222`、`I_S16=-16723/-8371.5`、`I_U20=266991`、`I_U4=3/0.6` | P0 | 通过 | 输出与预期逐项一致：`I_U8 raw:222 physical:222`、`I_S16 raw:-16723 physical:-8371.5`、`I_U20 raw:266991`、`I_U4 raw:3 physical:0.6000000000000001` |
| C09 | CAN/DBC 解码 | 夹具已生成 | Win11 · lumelog-can.exe release | `motorola.log`（`can0 0C8#7F80000000000000`）+ `bitorder.dbc` | `lumelog-can decode --log <T>\motorola.log --format candump --dbc <T>\bitorder.dbc --out jsonl` | Motorola 位序信号 | `M_U8=127`、`M_S16 raw -32768 / physical -3281.8` | P0 | 通过 | rc=0，`M_U8 raw:127 physical:127`、`M_S16 raw:-32768 physical:-3281.8`、`M_U20=0`、`M_U4=0 physical:1` |
| C10 | CAN/DBC 解码 | 夹具已复制 | Win11 · lumelog-can.exe release | `asc_basic.asc` + `sample.dbc` | `lumelog-can decode --log <T>\asc_basic.asc --format asc --dbc <T>\sample.dbc --out csv` | ASC 源解析 | `decoded:3`、`skipped:4`；CSV 表头 + 3 行 | P1 | 通过 | rc=0，CSV 为 `line,ts,channel,id,dlc,data` + `3,1,1,0x123,8,DEADBEEF00112233`、`4,2.5,2,0x1A7,4,11223344`、`5,3,1,0x321,3,000000`；`decoded=3 skipped=4` |
| C11 | CAN/DBC 解码 | 夹具已生成 | Win11 · lumelog-can.exe release | `asc_bad.asc`（1 合法帧 + `garbage line here` + ID `ZZZ`） | `lumelog-can decode --log <T>\asc_bad.asc --format asc --out jsonl` | ASC 坏行（异常） | 合法帧仍解出，坏行计入 skipped | P2 | 通过 | rc=0，`{"line":4,"id":"0x123","dlc":8,"data":"DEADBEEF12345678"}`，`{"decoded":1,"skipped":6}` |
| C12 | CAN/DBC 解码 | 夹具已生成 | Win11 · lumelog-can.exe release | `custom_regex2.log`；regex `T=(?P<ts>[0-9.]+) ID=(?P<id>0x[0-9A-Fa-f]+) DATA=(?P<data>[0-9A-Fa-f:\-]+)` | `lumelog-can decode --log <T>\custom_regex2.log --format regex --regex "…" --out jsonl` | 自定义正则源 | 2 帧解析成功，ts/id/data 正确 | P1 | 通过 | rc=0，`{"ts":12.345,"id":"0x1A2","dlc":4,"data":"11223344"}`、`{"ts":13.7,"id":"0x7FF","dlc":2,"data":"AABB"}`；`decoded:2 skipped:0` |
| C13 | CAN/DBC 解码 | 夹具已生成 | Win11 · lumelog-can.exe release | `custom_regex.log`；regex 仅含 `t` 组 | `lumelog-can decode --log <T>\custom_regex.log --format regex --regex "T=(?P<t>[0-9.]+)"` | 缺少必需命名组（异常） | rc=2，提示必须定义 `id`/`data` | P2 | 通过 | rc=2，stderr `decode: invalid regex: pattern must define named groups `id` and `data` (optional `ts`)` |
| C14 | CAN/DBC 解码 | 夹具已生成 | Win11 · lumelog-can.exe release | `custom_regex.log`；regex `(` | `lumelog-can decode --log <T>\custom_regex.log --format regex --regex "("` | 正则编译失败（异常） | rc=2，报 unclosed group | P2 | 通过 | rc=2，`decode: regex compile failed: regex parse error: … error: unclosed group` |
| C15 | CAN/DBC 解码 | 夹具已生成 | Win11 · lumelog-can.exe release | `bad_candump2.log`（奇数 hex、非 hex、非法 ID、65 字节、空行、1 合法帧） | `lumelog-can decode --log <T>\bad_candump2.log --format candump --out jsonl` | 多种坏帧（异常） | 合法帧解出，4 条坏帧 skipped，空行不计 | P1 | 通过 | rc=0，`{"line":6,"id":"0x456","dlc":2,"data":"1122"}`；`{"decoded":1,"skipped":4}` |
| C16 | CAN/DBC 解码 | 夹具已生成 | Win11 · lumelog-can.exe release | `emoji_payload.log`（`can0 123#😀`） | `lumelog-can decode --log <T>\emoji_payload.log --format candump --out jsonl` | 非 ASCII payload（异常/特殊字符） | rc≠0 但应为可控错误（skipped 或报错），不得 panic | P0 | 不通过 | rc=101，进程 panic：`thread 'main' (48496) panicked at crates\lumelog-can\src\source\mod.rs:33:39: end byte index 2 is not a char boundary; it is inside '😀' (bytes 0..4 of string)` |
| C17 | CAN/DBC 解码 | 夹具已生成 | Win11 · lumelog-can.exe release | `rtr_ext_fd.log`（`123#R`、`1FFFFFFF#42`、`123##03112233…`） | `lumelog-can decode --log <T>\rtr_ext_fd.log --format candump --out jsonl` | RTR/扩展 ID/CAN-FD（边界） | 3 帧解出；RTR dlc0、扩展 ID `0x1FFFFFFF`、FD 8 字节 | P1 | 通过 | rc=0，3 条：`0x123 dlc0 data""`、`0x1FFFFFFF dlc1 42`、`0x123 dlc8 1122334455667788`；`decoded:3 skipped:0` |
| C18 | CAN/DBC 解码 | 夹具已生成 | Win11 · lumelog-can.exe release | `empty.canlog`（0 字节日志） | `lumelog-can decode --log <T>\empty.canlog --format candump --out jsonl` | 空日志（边界） | `decoded:0`、`skipped:0`，不崩溃 | P2 | 通过 | rc=0，`{"decoded":0,"skipped":0}` |
| C19 | CAN/DBC 解码 | 夹具已复制 | Win11 · lumelog-can.exe release | `candump_basic.log` + `sample.dbc`；`--limit 0` | `lumelog-can decode … --limit 0` | limit=0 下边界 | 不打印帧，仍完成统计 | P2 | 通过 | rc=0，仅输出 `{"decoded":7,"skipped":2}` |
| C20 | CAN/DBC 解码 | 夹具已复制 | Win11 · lumelog-can.exe release | `candump_basic.log`；`--limit abc` | `lumelog-can decode … --limit abc` | 非数字 limit（异常） | rc=2，提示必须是非负整数 | P2 | 通过 | rc=2，`decode: --limit must be a non-negative integer (got abc)` |
| C21 | CAN/DBC 解码 | 夹具已复制 | Win11 · lumelog-can.exe release | `candump_basic.log`；`--format xml` | `lumelog-can decode … --format xml` | 非法格式（异常） | rc=2，提示只接受 candump/asc/regex | P2 | 通过 | rc=2，`decode: --format must be candump\|asc\|regex (got xml)` + usage |
| C22 | CAN/DBC 解码 | 无 | Win11 · lumelog-can.exe release | 仅 `--format candump`，缺 `--log` | `lumelog-can decode --format candump` | 缺少必填参数（异常） | rc=2，提示 `--log PATH is required` | P2 | 通过 | rc=2，`decode: --log PATH is required` + usage |
| C23 | CAN/DBC 解码 | 夹具已生成 | Win11 · lumelog-can.exe release | `ext_frame.log`（`can0 500#1122334455667788`）+ `sample.dbc` | `lumelog-can decode --log <T>\ext_frame.log --format candump --dbc <T>\sample.dbc --out jsonl` | 扩展帧 ID 掩码后匹配 DBC | 命中 `ExtFrame`，`SignalA raw:4386 physical:2183` | P1 | 通过 | rc=0，`{"id":"0x500","message_name":"ExtFrame","signals":[{"name":"SignalA","raw":4386,"physical":2183,"unit":"V"}]}` |
| C24 | CAN/DBC 解码 | 夹具已生成 | Win11 · lumelog-can.exe release | `p65.log`（`can0 123#` + `AB`×65，130 个 hex 字符） | `lumelog-can decode --log <T>\p65.log --format candump --out jsonl` | 65 字节 payload 上边界 | 拒绝该帧，`decoded:0`、`skipped:1` | P2 | 通过 | rc=0，`{"decoded":0,"skipped":1}` |
| C25 | CAN/DBC 解码 | 需 GUI/IPC；BLF 样例未提供 | Win11 · 无 GUI 会话 | BLF 样例（仓库未提供可直接使用的 `.blf` 夹具） | 通过 GUI 打开 BLF，观察报文表 | BLF 源解析 | 能打开并显示帧；或明确报不支持 | P1 | 未测试 | CLI 仅支持 `candump/asc/regex` 三种源；BLF 解析仅在 GUI/IPC 路径，且本次无 GUI 会话、仓库内无 `.blf` 夹具，未构造二进制 BLF（避免使用非真实格式数据） |

### 4.4 GUI 类功能（20 条）

> 说明：G 类中 G06–G11 为“行过滤/时序筛选引擎层”真实执行（Rust harness 直调 `lumelog-core`，不启 GUI）；G13–G16、G19 为前端纯函数套件真实执行（Vitest 无头）；其余交互层用例因本会话无交互桌面、无法向 Tauri WebView2 注入输入，标为“未测试”。

| 编号 | 模块 | 前置条件 | 测试环境 | 测试数据 | 测试步骤 | 测试要点 | 预期结果 | 用例级别(P0/P1/P2) | 测试结果 | 实际结果 |
|---|---|---|---|---|---|---|---|---|---|---|
| G01 | GUI 类功能 | 需交互桌面 | Win11 · Tauri WebView2 | `bench_8mb.log`（7,587,772B） | 启动 `lumelog-app.exe`，双击打开该文件，观察状态栏行号/编码 | 编辑器打开与状态栏 | 显示 UTF-8、7,587,772 字节、131,072 行，滚动流畅 | P0 | 未测试 | 本会话无交互桌面，无法启动并操作 Tauri 窗口；WebView2 不能由命令行/agent 注入键鼠事件 |
| G02 | GUI 类功能 | 需交互桌面 | Win11 · Tauri WebView2 | `bench_256mb.log`（242.8MB > 200MiB） | 打开文件，查看状态栏模式，拖滚动条到末尾 | >200MB 只读虚拟列表（边界） | 显示“只读”且不建全量索引，滚动按需取块、可到末尾 | P1 | 未测试 | 同上；只读阈值逻辑位于 `commands.rs` 的 `doc_open`，但无法在 GUI 观察实际模式 |
| G03 | GUI 类功能 | 需交互桌面 | Win11 · Tauri WebView2 | `big5_2.log` | 打开后将编码下拉从 GBK 改为 BIG5 | 编码下拉切换重解码 | 文本立即变为正确繁体中文，行号不变 | P1 | 未测试 | 同上；核心 `--encoding BIG5` 已在 E08 验证，但下拉 IPC `doc_set_encoding` 与重渲染未执行 |
| G04 | GUI 类功能 | 需交互桌面；文件被外部修改 | Win11 · Tauri WebView2 | `save_lf.log`；打开后外部写入 `external\n`，编辑器内改为 `mine\n` | 按 Ctrl+S，观察冲突对话框 | 保存冲突三选（异常） | 弹出“覆盖/另存/重新加载”，三选行为正确 | P1 | 未测试 | 同上；CLI `save` 无 conflict 三选交互（固定 `conflict:false`），需 GUI 验证 |
| G05 | GUI 类功能 | 需交互桌面 | Win11 · Tauri WebView2 | GBK 文档，输入含 `😀` | 保存，观察提示 | 不可映射字符提示（异常） | 提示 N 个字符不可映射，不静默丢字 | P2 | 未测试 | 同上；核心检测已在 E15 验证（1 个不可映射 + 拒绝保存），GUI 提示文案/计数未验证 |
| G06 | GUI 类功能 | harness 可执行 | Win11 · Rust harness + lumelog-core | 行 0 `2026-11-09 10:00:00 错误 E101 温度=95`；include `错误` + include `温度=9`，caseSensitive=true | 编译并运行 `lumelog-filter-harness.exe`，调用 `LineFilter::build` + `keep` | 多条件 AND + UTF-16 高亮区间 | 行 0 保留，ranges `[(20,2),(28,4)]`；行 1 丢弃 | P1 | 通过 | `F-01 … => PASS`；`l0=[(20, 2), (28, 4)] l1_kept=false l2=[(20, 2), (28, 4)]`；总结果 `RESULT PASS 8/8` |
| G07 | GUI 类功能 | 同上 | Win11 · Rust harness | include `错误` + exclude `E101` | 调用 `keep(0, …)` / `keep(2, …)` | 排除条件否决 | 行 0 丢弃，行 2 保留 | P1 | 通过 | `F-02 排除否决: line0_dropped=true line2_kept=true => PASS` |
| G08 | GUI 类功能 | 同上 | Win11 · Rust harness | `TimeSpec{extract:"(?<ts>\\d{4}-\\d{2}-\\d{2}[T ]\\d{2}:\\d{2}:\\d{2})", format:Iso, from:"2026-11-09 10:00:01", to:"2026-11-09 10:00:02", keep_when_no_ts:false}`；4 行（10:00:00 / :01 / :02 / 无时间戳） | 构造过滤器并 `keep` 四行 | ISO 时序窗口筛选 | 仅 :01、:02 保留，窗口外与无时间戳丢弃 | P1 | 通过 | `F-03 ISO 时序窗口: out0=true in1=true in2=true no_ts_dropped=true => PASS` |
| G09 | GUI 类功能 | 同上 | Win11 · Rust harness | `line_range=(2,3)`，无正则条件；4 行文本 `x` | 构造过滤器并 `keep(0..3)` | 行号区间 1-based 闭区间（边界） | 第 2、3 行保留；第 1、4 行丢弃 | P1 | 通过 | `F-04 行号区间[2,3]: 1=false 2=true 3=true 4=false => PASS` |
| G10 | GUI 类功能 | 同上 | Win11 · Rust harness | include 条件文本 `""` | `LineFilter::build([空条件], …)` | 空过滤条件（异常） | 返回 Err，不进入扫描 | P2 | 通过 | `F-05 空过滤条件: err=Some("invalid search pattern: empty filter condition") => PASS` |
| G11 | GUI 类功能 | 同上 | Win11 · Rust harness + Vitest | 过滤正则 `(`；另 `pickTimelineInput`：`from="5",to=""`、`from="9",to="2"`、`from="a",to="5"` | 运行 harness F-06；运行 `npx vitest run src/query.test.ts` | 非法正则/非法行区间（异常） | 均返回 error，不抛异常 | P1 | 通过 | harness `F-06 非法正则条件: err=true => PASS`；query 套件 7/7 通过，其中 `line_range_requires_both_sides`、`line_range_reversed_errors`、`line_range_non_integer_errors` 均得到 `error` |
| G12 | GUI 类功能 | 需交互桌面 | Win11 · Tauri WebView2 | `bench_256mb.log`，搜索 `val=999` | 点击搜索后 0.5s 内点“取消” | 搜索取消 | 查询面板显示“已取消”，已扫描字节小于全文件 | P1 | 未测试 | 同上；CLI 无取消入口，取消仅在 GUI/IPC `search_cancel` 路径 |
| G13 | GUI 类功能 | node_modules 已安装 | Win11 · Vitest 无头 | `resolveTheme("light", true/false)`、`dark`、`vampire`、`auto`；`nextPref` 循环 light→dark→vampire→auto→light | `npx vitest run src/theme.test.ts` | 主题解析与循环 | 4/4 通过：显式主题不受系统影响，auto 跟随系统，循环回到自身 | P2 | 通过 | `theme.test.ts 4/4 passed`；Vitest 汇总 40/40 |
| G14 | GUI 类功能 | node_modules 已安装 | Win11 · Vitest 无头 | `Debouncer(2000ms)`，`schedule(7)` 三次；`cancel(1)`；`cancelAll`；`humanSize(1536)`、`humanSize(NaN)` | `npx vitest run src/recovery.test.ts` | 崩溃恢复影子写入节流 | 5/5 通过：1999ms 不写、2000ms 只写一次；cancel 丢弃；NaN 显示 `0 B` | P1 | 通过 | `recovery.test.ts 5/5 passed` |
| G15 | GUI 类功能 | node_modules 已安装 | Win11 · Vitest 无头 | 路径 `["a.log","b.log","a.log","","c.log"]`；60 条 `f0.log…f59.log`；`active:"ghost.log"`；`parseSession("{oops")`、`"42"` | `npx vitest run src/session.test.ts` | 会话恢复去重/上限/脏数据（边界） | 4/4 通过：去重为 3 条、上限 50、ghost active→null、脏 JSON→空会话 | P1 | 通过 | `session.test.ts 4/4 passed`；`paths` 长度 50、`active:null` |
| G16 | GUI 类功能 | node_modules 已安装 | Win11 · Vitest 无头 | `validate("S")`、`Escape`、`F3`、`Shift+F3`、`Ctrl+F`、`Ctrl+Shift+F`、`Alt+1`；`mergeBindings({save:"Ctrl+K", close:"Ctrl+F4", open:"", save:"S", save:"F3"})` | `npx vitest run src/keymap.test.ts src/keymap-ui.test.ts` | 快捷键保留键/冲突/非法值（异常） | 13/13 通过：拒绝裸 `S`、F3、Ctrl+F；接受 Ctrl+Shift+F；非法/保留值回落默认 | P2 | 通过 | `keymap.test.ts 11/11`、`keymap-ui.test.ts 2/2`；合并后 `save:"Ctrl+S"`、`invalid:["save"]` |
| G17 | GUI 类功能 | 需交互桌面 | Win11 · Tauri WebView2 | `readonly.log` 或 `save_lf.log` | 外部修改文件，观察编辑器提示 | 外部修改监听 | 干净文档自动重载；脏文档显示黄点/提示，不静默覆盖 | P1 | 未测试 | 同上；文件监听依赖 Tauri 事件循环与窗口焦点，无 GUI 会话 |
| G18 | GUI 类功能 | 需交互桌面 | Win11 · Tauri WebView2 | `gbk.log` 过滤命中 1 行；导出 `filtered.csv`/`filtered.log` | 过滤后点“导出”，检查落盘文件 | 导出编码/行尾/CSV 上限 | 导出内容与筛选一致，编码/行尾按原文件保持 | P2 | 未测试 | 同上；CAN CSV 导出已在 C10 验证，GUI 查询面板导出未执行 |
| G19 | GUI 类功能 | node_modules 已安装 | Win11 · Vitest 无头 | 4 帧（ID 0x100/0x1FF/0x300/0x150，ts 1500/2000/5000/undefined）；`idFrom:0x100,idTo:0x1FF`、`idFrom:0x1FF,idTo:0x100`、`msgName:"EngineData"`、`signalName:"Speed", op:">", value:60` | `npx vitest run src/canfilter.test.ts` | CAN 帧过滤 | 7/7 通过：闭区间含首尾、反向区间为空、消息名精确、信号比较正确 | P2 | 通过 | `canfilter.test.ts 7/7`；正区间返回行 1,2,4，反向区间返回 `[]` |
| G20 | GUI 类功能 | 需交互桌面与文件关联注册 | Win11 · Tauri WebView2 | 两个 `lumelog-app.exe` 进程；双击 `.log` 文件 | 启动两次应用；双击已关联文件 | 单实例/文件关联 | 第二次启动转发给已有实例并打开文件，不产生双窗口 | P2 | 未测试 | 同上；需注册文件关联并在真实桌面双击，无法在本会话执行 |

## 5. 未通过用例汇总

| 编号 | 模块 | 用例要点 | 实际结果 | 严重程度 | 疑似原因 |
|---|---|---|---|---|---|
| C16 | CAN/DBC 解码 | 非 ASCII payload（`can0 123#😀`）应可控报错/skip | rc=101，panic：`byte index 2 is not a char boundary`，位置 `crates/lumelog-can/src/source/mod.rs:33` | 高 | `hex_to_bytes` 按字节偏移 `s[i..i+2]` 切片；非 ASCII 字符跨 UTF-8 char 边界即 panic，与 `source/mod.rs` 中“without ever panicking”的注释不符 |
| E07 | 编码识别与解码 | Big5 自动识别 | 识别为 GBK，首行乱码；手动 BIG5 正常 | 中 | `encoding/mod.rs` 的 `detect` 在 cn hint 下得到 GBK(plausible)，无 hint 复核只采纳 EUC-JP/EUC-KR（第 165–175 行），Big5 保留 hinted GBK |
| E09 | 编码识别与解码 | Shift-JIS 自动识别 | 识别为 GBK，首行乱码；手动 Shift-JIS 正常 | 中 | 同上：无 hint 复核未采纳 Shift-JIS；chardetng 在中文 hint 下返回 GBK |
| B05 | 大文件、行索引与搜索 | 8MB 忽略大小写搜索性能 | 命中 131,072 条，`--max 20` 仍耗时 48.98s | 中 | 忽略大小写走正则/逐命中路径，并逐条构造 snippet（`chunk_at`/行定位）；对照大小写敏感 0 命中时 0.05s |
| B22 | 大文件、行索引与搜索 | 未知选项应被拒绝 | 传入 `--mode fuzzy`，rc=0，选项被静默忽略 | 低 | CLI 参数解析未识别/未校验未知选项；`search` 实际只支持 `--regex`，不存在 `--mode`，未知 token 被丢弃 |
| E23 | 编码识别与解码 | 只读文件保存应提示权限/只读 | 保存被拒绝，但提示 `target file is locked by another process` | 低 | `doc/save.rs` 的 `is_locked` 把 `PermissionDenied(Os 5)` 也归类为“被其他进程占用”，错误文案与真实原因不符 |

## 6. 未测试清单与原因

| 编号 | 用例要点 | 未测试原因 |
|---|---|---|
| B19 | 搜索取消 | CLI `search` 无取消参数；`cancelled` 由 `search_stream` 的 progress 回调/IPC `search_cancel` 驱动，需要 GUI 会话注入点击 |
| B20 | >200MB 只读虚拟列表 | 只读模式在 `doc_open`（GUI/IPC）中按 `size > 200MiB` 返回；CLI 一律全量索引，无法观察 GUI 模式与按需取块 |
| C25 | BLF 源解析 | CLI 只支持 candump/asc/regex；BLF 仅 GUI/IPC，仓库无 `.blf` 夹具，未伪造二进制格式数据 |
| G01 | 编辑器打开与状态栏 | 无交互桌面，无法启动/操作 Tauri WebView2；agent 不能注入键鼠 |
| G02 | 大文件只读虚拟列表 | 同上 |
| G03 | 编码下拉切换重解码 | 同上（核心解码已在 E08 验证，IPC/重渲染未验证） |
| G04 | 保存冲突三选 | 同上；冲突对话框需真实外部修改与 GUI 交互 |
| G05 | 不可映射提示文案/计数 | 同上（核心检测已在 E15 验证） |
| G12 | 查询面板搜索取消 | 同上 |
| G17 | 外部修改监听 | 同上；依赖 Tauri 事件循环与窗口焦点 |
| G18 | 查询面板导出日志/CSV | 同上（CAN CSV 已在 C10 验证） |
| G20 | 单实例/文件关联 | 同上；需注册文件关联并在真实桌面双击 |

## 7. 结论与建议

- 核心能力（编码检测/解码、大文件索引与字面搜索、DBC 宽松解析、Intel/Motorola 解码、candump/ASC/正则源、坏帧跳过、参数错误处理）整体稳定：已执行 80 条中 74 条通过（92.5%）。
- 必须优先修复 **C16 非 ASCII payload panic**（P0/高）：一行含任意非 ASCII 字符的 candump 报文即可让进程崩溃，违反“解析永不 panic”的契约。
- 其次处理 **Big5/Shift-JIS 自动识别退化到 GBK**（E07/E09）与 **忽略大小写搜索大量命中时的性能**（B05）；前者影响用户直接打开日志的正确性，后者会让 8MB 日志的查询长时间无响应。
- 参数校验类问题（B22 未知选项被静默忽略、E23 只读文件错误文案）为低severity，但会误导用户/掩盖真实失败。
- 本次未改动任何仓库代码；所有夹具与 harness 均位于 `%TEMP%` 下。GUI 交互用例需在具备交互桌面的环境补测，重点是 G02/G03/G04/G12/G17/G20。
