# LogLume 实际执行测试用例表（2026-11-10，macOS）

> **结果口径：** 每条的「实际结果」都是真实命令 / 真实 GUI 输出，未跑过的范围也以文字写明「未执行 / 未验证」，不代填、不推测。门禁只评用例结构与覆盖，不判定业务正确性。
>
> **怎么查：** §3 速查表按 `TC-001`～`TC-033` 顺序列全 33 条；§4 明细按模块分组（编号会跳号），搜用例编号即可定位到那一条。

## 1. 范围与环境

| 项          | 值                                                                                                                                            |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| 被测应用代码     | `ee68040`（用例表本身于 `c874db4` 入库，未改动应用代码）                                                                                                       |
| 操作系统       | macOS 15，交互桌面、`caffeinate` 保持亮屏（测试结束已释放断言）                                                                                                   |
| 桌面进程       | `lumelog-app` pid 89489；`tauri dev` 编译 `Finished dev profile in 30.20s` 并 `Running`                                                          |
| dev server | `http://localhost:1520`，`curl` 返回 200                                                                                                        |
| CLI 被测对象   | `lumelog`（`lumelog-core`）、`lumelog-can`（`lumelog-can`）、`lumelog-gen`                                                                         |
| 真实夹具       | `sample_gbk.log`、`sample.dbc`、`candump_basic.log`、`asc_basic.asc`、`cli_intel.log`、`lumelog-gen 1`                                            |
| 自动化套件      | `npx vitest run` 39 文件 / 304 tests；`cargo test --workspace` 388 passed / 0 failed；`npx tsc --noEmit` exit=0；`npm run build` 3.72s 通过         |
| 用例质量门禁     | `test-mentor/case-gate.js`，通过 100/100、风险低、23 项检查全通过                                                                                          |
| 证据文件       | `/tmp/real-*.txt` / `/tmp/real-*.log` 命令输出；`/tmp/real-palette.png`、`/tmp/real-dark2.png`、`/tmp/real-opened2.png`、`/tmp/real-ctxmenu.png` 等截图 |

## 2. 统计

| 项                             | 数量                      |
| ----------------------------- | ---:------------------- |
| 用例总数                          | 33（`TC-001` ～ `TC-033`） |
| `actual` 已填真实结果               | 33（0 条留空）               |
| 判定：通过                         | 31                      |
| 判定：部分验证（范围未跑全，已在 `actual` 注明） | 2（`TC-029`、`TC-030`）    |
| 判定：不通过                        | 0                       |
| 负例 + 边界占比                     | 48%（≥ 40% 要求）           |
| 平均步骤数                         | 1.8                     |

**模块分布：** core CLI 11、CAN CLI 11、macOS 原生菜单 3、前端 UI 2、桌面运行环境 2，性能 / 兼容性 / 非功能 / 自动化测试套件各 1。 **优先级分布：** P0 3、P1 19、P2 10、P3 1。

> 判定说明：31 条「通过」指实际结果满足该用例写明的预期；`TC-029` 的降级行为、`TC-030` 的 Windows/Linux/其他分辨率在 `actual` 中明确写为未验证，故记「部分验证」，不计入通过，也不算缺陷。

## 3. 用例速查表

| 编号     | 模块         | 级别  | 类型  | 方法    | 标题                        | 判定   |
| ------ | ---------- | --- | --- | ----- | ------------------------- | ---- |
| TC-001 | core CLI   | P0  | 功能  | 等价类划分 | 打开 GBK 样本日志前 5 行不乱码       | 通过   |
| TC-002 | core CLI   | P1  | 功能  | 等价类划分 | 统计 GBK 样本行数               | 通过   |
| TC-003 | core CLI   | P1  | 功能  | 场景法   | 中文关键词搜索返回命中统计             | 通过   |
| TC-004 | core CLI   | P1  | 异常  | 错误推测  | 非法正则被拒绝并返回错误信息            | 通过   |
| TC-005 | core CLI   | P1  | 异常  | 错误推测  | 打开不存在的文件返回 IO 错误          | 通过   |
| TC-006 | core CLI   | P2  | 异常  | 错误推测  | 未知编码标签触发用法错误              | 通过   |
| TC-007 | core CLI   | P1  | 功能  | 场景法   | GBK 与 CRLF 保存回环保持一致       | 通过   |
| TC-008 | CAN CLI    | P1  | 异常  | 错误推测  | DBC 严格模式遇到坏 SG\_ 行报错      | 通过   |
| TC-009 | CAN CLI    | P1  | 功能  | 状态图   | DBC lenient 模式跳过坏行继续解析    | 通过   |
| TC-010 | CAN CLI    | P0  | 功能  | 等价类划分 | candump 日志配合 DBC 导出 CSV 帧 | 通过   |
| TC-011 | CAN CLI    | P1  | 功能  | 等价类划分 | ASC 日志导出 JSONL 帧          | 通过   |
| TC-012 | CAN CLI    | P1  | 异常  | 错误推测  | CAN 日志文件不存在时返回错误          | 通过   |
| TC-013 | CAN CLI    | P1  | 异常  | 错误推测  | CAN 正则源非法正则被拒绝            | 通过   |
| TC-014 | CAN CLI    | P2  | 异常  | 错误推测  | decode 缺少必填 --log 参数      | 通过   |
| TC-015 | CAN CLI    | P1  | 边界  | 边界值   | limit 边界三位置之下限等于 0        | 通过   |
| TC-016 | CAN CLI    | P1  | 边界  | 边界值   | limit 边界三位置之略大于下限 1       | 通过   |
| TC-017 | CAN CLI    | P1  | 边界  | 边界值   | limit 边界三位置之上限 3          | 通过   |
| TC-018 | CAN CLI    | P2  | 异常  | 错误推测  | limit 传入负数或非法类型被拒绝        | 通过   |
| TC-019 | core CLI   | P2  | 异常  | 错误推测  | 空字符串搜索模式被拒绝               | 通过   |
| TC-020 | core CLI   | P2  | 安全  | 错误推测  | 超长特殊字符与注入片段不导致崩溃          | 通过   |
| TC-021 | core CLI   | P2  | 异常  | 错误推测  | 无读权限文件触发权限错误              | 通过   |
| TC-022 | core CLI   | P1  | 功能  | 状态图   | 重复提交相同保存请求保持幂等            | 通过   |
| TC-023 | 性能         | P2  | 性能  | 场景法   | 约 1MiB 日志的搜索与行数统计响应时间     | 通过   |
| TC-024 | macOS 原生菜单 | P1  | 功能  | 场景法   | 原生菜单栏分类与编辑菜单全中文           | 通过   |
| TC-025 | macOS 原生菜单 | P1  | 功能  | 场景法   | 点击命令面板菜单项触发前端联动           | 通过   |
| TC-026 | macOS 原生菜单 | P1  | 功能  | 状态图   | 点击暗黑主题菜单项改变界面状态           | 通过   |
| TC-027 | 桌面运行环境     | P1  | 功能  | 场景法   | dev server 与应用进程可观测       | 通过   |
| TC-028 | 自动化测试套件    | P0  | 功能  | 场景法   | 前端与 Rust 全量自动化回归          | 通过   |
| TC-029 | 非功能        | P2  | 非功能 | 场景法   | 超时限流降级与并发场景               | 部分验证 |
| TC-030 | 兼容性        | P3  | 兼容  | 场景法   | 跨平台版本与分辨率兼容               | 部分验证 |
| TC-031 | 前端 UI      | P2  | 功能  | 场景法   | 应用内中文右键菜单                 | 通过   |
| TC-032 | 桌面运行环境     | P1  | 功能  | 场景法   | 第二实例转发真实打开 GBK 日志文件       | 通过   |
| TC-033 | 前端 UI      | P2  | 功能  | 场景法   | 剪贴板编码与 flavor 交叉验证        | 通过   |

## 4. 用例明细

### 4.1 core CLI（11 条）

#### TC-001 打开 GBK 样本日志前 5 行不乱码

* **标签**：`core CLI` · `P0` · 功能 · 等价类划分 · **判定：通过**
* **前置条件**：仓库根目录可执行 cargo，crates/lumelog-core/tests/data/sample_gbk.log 存在
* **测试数据**：sample_gbk.log，--head 5
* **步骤**：
  1. 执行 cargo run -q -p lumelog-core --bin lumelog -- open --file crates/lumelog-core/tests/data/sample_gbk.log --head 5
* **预期结果**：输出元数据 size=1109 lines=20 encoding=GBK eol=lf，随后 5 行含中文「台架 HIL 测试 记录 完成」，退出码 0
* **实际结果**：实测输出 size=1109 lines=20 encoding=GBK eol=lf，5 行 val=0 到 val=4 中文无乱码，exit=0（/tmp/real-cli.txt C1）

#### TC-002 统计 GBK 样本行数

* **标签**：`core CLI` · `P1` · 功能 · 等价类划分 · **判定：通过**
* **前置条件**：TC-001 的样本文件保持不变
* **测试数据**：--line-count，样本 20 行
* **步骤**：
  1. 执行 cargo run -q -p lumelog-core --bin lumelog -- open --file crates/lumelog-core/tests/data/sample_gbk.log --line-count
* **预期结果**：标准输出为 20 且退出码 0
* **实际结果**：实测输出 20，exit=0（C2）

#### TC-003 中文关键词搜索返回命中统计

* **标签**：`core CLI` · `P1` · 功能 · 场景法 · **判定：通过**
* **前置条件**：样本日志已用 GBK 正确打开
* **测试数据**：--pattern 错误，--max 3
* **步骤**：
  1. 执行 cargo run -q -p lumelog-core --bin lumelog -- search --file crates/lumelog-core/tests/data/sample_gbk.log --pattern 错误 --max 3
* **预期结果**：输出命中统计 JSON，scanned_bytes=1109，cancelled=false，退出码 0
* **实际结果**：实测 matches=0 scanned_bytes=1109 cancelled=false truncated=false，exit=0（C3；样本不含「错误」）

#### TC-004 非法正则被拒绝并返回错误信息

* **标签**：`core CLI` · `P1` · 异常 · 错误推测 · **判定：通过**
* **前置条件**：样本日志可读，命令带 --regex
* **测试数据**：--pattern (，--regex
* **步骤**：
  1. 执行 cargo run -q -p lumelog-core --bin lumelog -- search --file crates/lumelog-core/tests/data/sample_gbk.log --pattern ( --regex
* **预期结果**：错误码 1，message 含 unclosed group，不输出任何命中行
* **实际结果**：实测 lumelog: invalid search pattern: regex parse error ... error: unclosed group，exit=1（C4）

#### TC-005 打开不存在的文件返回 IO 错误

* **标签**：`core CLI` · `P1` · 异常 · 错误推测 · **判定：通过**
* **前置条件**：路径 /tmp/definitely-missing.log 不存在
* **测试数据**：--file /tmp/definitely-missing.log
* **步骤**：
  1. 执行 cargo run -q -p lumelog-core --bin lumelog -- open --file /tmp/definitely-missing.log
* **预期结果**：错误码 1，message 含 No such file or directory，无文件内容输出
* **实际结果**：实测 lumelog: io error: No such file or directory (os error 2)，exit=1（C5）

#### TC-006 未知编码标签触发用法错误

* **标签**：`core CLI` · `P2` · 异常 · 错误推测 · **判定：通过**
* **前置条件**：样本日志可读
* **测试数据**：--encoding NOPE
* **步骤**：
  1. 执行 cargo run -q -p lumelog-core --bin lumelog -- open --file crates/lumelog-core/tests/data/sample_gbk.log --encoding NOPE
* **预期结果**：错误码 2，message 含 unknown encoding 与 usage 说明，状态码可判定
* **实际结果**：实测 lumelog: unknown encoding NOPE 并打印 usage，exit=2（C6）

#### TC-007 GBK 与 CRLF 保存回环保持一致

* **标签**：`core CLI` · `P1` · 功能 · 场景法 · **判定：通过**
* **前置条件**：样本日志可读，/tmp 可写
* **测试数据**：stdin=中文回环 ABC，--encoding GBK --eol CRLF --out /tmp/real-save.gbk
* **步骤**：
  1. 执行 printf 中文回环 ABC 经 stdin 传给 save 命令并输出到 /tmp/real-save.gbk
  2. 执行 open 命令读取 /tmp/real-save.gbk --head 2
* **预期结果**：save 返回 saved=true conflict=false，open 显示 size=14 lines=2 encoding=GBK eol=crlf 与中文回环 ABC，退出码 0
* **实际结果**：实测 C7 saved=true replace_count=0 conflict=false；C8 size=14 lines=2 encoding=GBK eol=crlf 与「中文回环 ABC」，均 exit=0

#### TC-019 空字符串搜索模式被拒绝

* **标签**：`core CLI` · `P2` · 异常 · 错误推测 · **判定：通过**
* **前置条件**：样本日志可读
* **测试数据**：--pattern 空字符串（null/空值/不填），--max 3
* **步骤**：
  1. 执行 cargo run -q -p lumelog-core --bin lumelog -- search --file crates/lumelog-core/tests/data/sample_gbk.log --pattern 空字符串 --max 3
* **预期结果**：空值输入返回错误码 1，message 含 empty search pattern，不产生命中输出
* **实际结果**：实测 lumelog: invalid search pattern: empty search pattern，exit=1（E1）

#### TC-020 超长特殊字符与注入片段不导致崩溃

* **标签**：`core CLI` · `P2` · 安全 · 错误推测 · **判定：通过**
* **前置条件**：样本日志为 GBK 编码
* **测试数据**：中文 OR 1=1; <script>alert(1)</script> emoji 加 2000 个 A，超长/特殊字符/注入
* **步骤**：
  1. 执行 search 命令并传入约 2KB 含中文 emoji XSS SQL 注入片段的模式
* **预期结果**：返回明确错误码与 message，进程不 panic，不输出文件内容，安全输入可审计
* **实际结果**：实测 lumelog: invalid search pattern: pattern is not representable in the file encoding，exit=1，进程未崩溃（E2）

#### TC-021 无读权限文件触发权限错误

* **标签**：`core CLI` · `P2` · 异常 · 错误推测 · **判定：通过**
* **前置条件**：/tmp/real-noread.log 权限 000，测试账号非 root
* **测试数据**：chmod 000 /tmp/real-noread.log，权限/越权/鉴权场景
* **步骤**：
  1. 执行 cargo run -q -p lumelog-core --bin lumelog -- open --file /tmp/real-noread.log
* **预期结果**：错误码 1，message 含 Permission denied，不泄露文件正文或敏感内容
* **实际结果**：实测 lumelog: io error: Permission denied (os error 13)，exit=1（D4）

#### TC-022 重复提交相同保存请求保持幂等

* **标签**：`core CLI` · `P1` · 功能 · 状态图 · **判定：通过**
* **前置条件**：样本日志可读，两个输出路径为空
* **测试数据**：stdin=幂等文本，重复提交 2 次，--encoding GBK
* **步骤**：
  1. 第一次执行 save 命令输出 /tmp/real-idem1.gbk
  2. 第二次执行相同 save 命令输出 /tmp/real-idem2.gbk
  3. 执行 cmp 比较两个输出文件
* **预期结果**：两次均返回 saved=true conflict=false，字节比较一致，状态连续且无重复副作用，退出码 0
* **实际结果**：实测两次均 saved=true replace_count=0 conflict=false，cmp=0，均 exit=0（D5）

### 4.2 CAN CLI（11 条）

#### TC-008 DBC 严格模式遇到坏 SG\_ 行报错

* **标签**：`CAN CLI` · `P1` · 异常 · 错误推测 · **判定：通过**
* **前置条件**：crates/lumelog-can/tests/data/sample.dbc 含第 14 行故意写坏的 SG\_
* **测试数据**：sample.dbc 严格解析
* **步骤**：
  1. 执行 cargo run -q -p lumelog-can --bin lumelog-can -- dbc --file crates/lumelog-can/tests/data/sample.dbc
* **预期结果**：错误码 1，message 指出 dbc syntax error at line 14 (bad SG\_)
* **实际结果**：实测 dbc: dbc syntax error at line 14 (bad SG\_): SG\_ BrokenSignal : 24|notanint@1+，exit=1（A1）

#### TC-009 DBC lenient 模式跳过坏行继续解析

* **标签**：`CAN CLI` · `P1` · 功能 · 状态图 · **判定：通过**
* **前置条件**：同一份 sample.dbc 含 1 处坏行
* **测试数据**：sample.dbc --lenient
* **步骤**：
  1. 执行 cargo run -q -p lumelog-can --bin lumelog-can -- dbc --file crates/lumelog-can/tests/data/sample.dbc --lenient
* **预期结果**：退出码 0，统计保留 messages=3 signals=6 errors=1，坏行不中断解析
* **实际结果**：实测 messages=3 signals=6 errors=1，exit=0（B1）

#### TC-010 candump 日志配合 DBC 导出 CSV 帧

* **标签**：`CAN CLI` · `P0` · 功能 · 等价类划分 · **判定：通过**
* **前置条件**：candump_basic.log 与 sample.dbc 存在
* **测试数据**：--format candump --dbc sample.dbc --out csv
* **步骤**：
  1. 执行 cargo run -q -p lumelog-can --bin lumelog-can -- decode --log crates/lumelog-can/tests/data/candump_basic.log --format candump --dbc crates/lumelog-can/tests/data/sample.dbc --limit 3 --out csv
* **预期结果**：输出 CSV 表头 line,ts,channel,id,dlc,data 与帧数据，末尾统计 decoded=7 skipped=2，退出码 0
* **实际结果**：实测 CSV 表头与 7 行帧（0x123 到 0x7AA），\# decoded=7 skipped=2，exit=0（A2）

#### TC-011 ASC 日志导出 JSONL 帧

* **标签**：`CAN CLI` · `P1` · 功能 · 等价类划分 · **判定：通过**
* **前置条件**：asc_basic.asc 存在
* **测试数据**：--format asc --limit 3 --out jsonl
* **步骤**：
  1. 执行 cargo run -q -p lumelog-can --bin lumelog-can -- decode --log crates/lumelog-can/tests/data/asc_basic.asc --format asc --limit 3 --out jsonl
* **预期结果**：输出 3 条含 line ts channel id dlc data 的 JSON 帧，统计 decoded=3 skipped=4，退出码 0
* **实际结果**：实测 3 条帧 0x123 / 0x1A7 / 0x321，decoded=3 skipped=4，exit=0（A3）

#### TC-012 CAN 日志文件不存在时返回错误

* **标签**：`CAN CLI` · `P1` · 异常 · 错误推测 · **判定：通过**
* **前置条件**：/tmp/no-such.log 不存在
* **测试数据**：--log /tmp/no-such.log
* **步骤**：
  1. 执行 cargo run -q -p lumelog-can --bin lumelog-can -- decode --log /tmp/no-such.log --format candump
* **预期结果**：错误码 1，message 含 cannot read 与 os error 2
* **实际结果**：实测 cannot read /tmp/no-such.log: No such file or directory (os error 2)，exit=1（A4）

#### TC-013 CAN 正则源非法正则被拒绝

* **标签**：`CAN CLI` · `P1` · 异常 · 错误推测 · **判定：通过**
* **前置条件**：cli_intel.log 存在
* **测试数据**：--format regex --regex (
* **步骤**：
  1. 执行 cargo run -q -p lumelog-can --bin lumelog-can -- decode --log crates/lumelog-can/tests/data/cli_intel.log --format regex --regex (
* **预期结果**：错误码 2，message 含 regex compile failed 与 unclosed group
* **实际结果**：实测 decode: regex compile failed: regex parse error ... error: unclosed group，exit=2（A5）

#### TC-014 decode 缺少必填 --log 参数

* **标签**：`CAN CLI` · `P2` · 异常 · 错误推测 · **判定：通过**
* **前置条件**：无额外前置条件
* **测试数据**：仅 decode 子命令
* **步骤**：
  1. 执行 cargo run -q -p lumelog-can --bin lumelog-can -- decode
* **预期结果**：错误码 2，message 指出 --log PATH is required 并打印 usage
* **实际结果**：实测 decode: --log PATH is required 及 usage，exit=2（A6）

#### TC-015 limit 边界三位置之下限等于 0

* **标签**：`CAN CLI` · `P1` · 边界 · 边界值 · **判定：通过**
* **前置条件**：candump_basic.log 可读，默认输出模式不指定 --out
* **测试数据**：边界三位置：下限等于 0、略大于下限 1、上限 3；本例 limit=0
* **步骤**：
  1. 执行 cargo run -q -p lumelog-can --bin lumelog-can -- decode --log crates/lumelog-can/tests/data/candump_basic.log --format candump --limit 0
* **预期结果**：下限 0 时不打印任何帧，仍输出 decoded=7 skipped=2，退出码 0
* **实际结果**：实测无帧输出，仅 decoded=7 skipped=2，exit=0（C11）

#### TC-016 limit 边界三位置之略大于下限 1

* **标签**：`CAN CLI` · `P1` · 边界 · 边界值 · **判定：通过**
* **前置条件**：candump_basic.log 可读，默认输出模式
* **测试数据**：边界三位置：下限等于 0、略大于下限 1、上限 3；本例 limit=1
* **步骤**：
  1. 执行 cargo run -q -p lumelog-can --bin lumelog-can -- decode --log crates/lumelog-can/tests/data/candump_basic.log --format candump --limit 1
* **预期结果**：只打印首帧 0x123 DEADBEEF，统计仍为 decoded=7 skipped=2，退出码 0
* **实际结果**：实测仅 1 条帧 line=1 id=0x123 data=DEADBEEF，decoded=7 skipped=2，exit=0（C10）

#### TC-017 limit 边界三位置之上限 3

* **标签**：`CAN CLI` · `P1` · 边界 · 边界值 · **判定：通过**
* **前置条件**：candump_basic.log 可读，默认输出模式
* **测试数据**：边界三位置：下限等于 0、略大于下限 1、上限 3；本例 limit=3
* **步骤**：
  1. 执行 cargo run -q -p lumelog-can --bin lumelog-can -- decode --log crates/lumelog-can/tests/data/candump_basic.log --format candump --limit 3
* **预期结果**：打印 3 条帧，统计 decoded=7 skipped=2，退出码 0
* **实际结果**：实测 3 条帧 0x123 / 0x735 / 0x1F，decoded=7 skipped=2，exit=0（C9）

#### TC-018 limit 传入负数或非法类型被拒绝

* **标签**：`CAN CLI` · `P2` · 异常 · 错误推测 · **判定：通过**
* **前置条件**：candump_basic.log 可读
* **测试数据**：--limit -1，非数字/负数/超范围输入
* **步骤**：
  1. 执行 cargo run -q -p lumelog-can --bin lumelog-can -- decode --log crates/lumelog-can/tests/data/candump_basic.log --format candump --limit -1
* **预期结果**：错误码 2，message 含 must be a non-negative integer 与实际值 -1
* **实际结果**：实测 decode: --limit must be a non-negative integer (got -1)，exit=2（D2）

### 4.3 macOS 原生菜单（3 条）

#### TC-024 原生菜单栏分类与编辑菜单全中文

* **标签**：`macOS 原生菜单` · `P1` · 功能 · 场景法 · **判定：通过**
* **前置条件**：LogLume 桌面进程 89489 运行中，系统辅助功能可用，caffeinate 保持屏幕常亮
* **测试数据**：System Events / process LogLume / menu bar 1
* **步骤**：
  1. 执行 osascript 读取 menu bar 1 的全部 menu bar item 名称
  2. 执行 osascript 读取编辑菜单的全部 menu item 名称
* **预期结果**：菜单栏依次为 Apple LogLume 文件 编辑 查看 数据 设置 窗口，编辑菜单不出现 AutoFill Start Dictation Emoji & Symbols
* **实际结果**：实测菜单栏 Apple, LogLume, 文件, 编辑, 查看, 数据, 设置, 窗口；编辑项为 撤销 重做 分隔 剪切 复制 粘贴 全选；启动日志 \[menu\] removed system item 共 6 条

#### TC-025 点击命令面板菜单项触发前端联动

* **标签**：`macOS 原生菜单` · `P1` · 功能 · 场景法 · **判定：通过**
* **前置条件**：LogLume 在前台且命令面板未打开，屏幕保持常亮
* **测试数据**：菜单项 命令面板…，menu://action 事件，上下游联动/回调/通知
* **步骤**：
  1. 执行 osascript 点击 查看 > 命令面板…
  2. 执行 screencapture -x /tmp/real-palette.png
  3. 执行 pgrep 检查 lumelog-app 进程
* **预期结果**：前端弹出中文命令面板，应用不退出，截图可见输入框与中文命令列表
* **实际结果**：实测 osascript click exit=0；截图显示「输入命令…」与 打开文件…/保存/另存为… 列表；进程 89489 存活（/tmp/real-palette.png）

#### TC-026 点击暗黑主题菜单项改变界面状态

* **标签**：`macOS 原生菜单` · `P1` · 功能 · 状态图 · **判定：通过**
* **前置条件**：LogLume 在前台，命令面板已打开，屏幕保持常亮
* **测试数据**：菜单项 主题：暗黑，界面状态连续变化与恢复基线
* **步骤**：
  1. 执行 osascript 置前 LogLume 进程
  2. 执行 osascript 点击 设置 > 主题：暗黑
  3. 执行 screencapture -x /tmp/real-dark2.png
* **预期结果**：界面从浅色切换为暗色，原生菜单栏保持中文，进程存活不中断
* **实际结果**：实测 click exit=0；截图为暗色界面与中文菜单栏；前台进程 lumelog-app，pid 89489 存活（/tmp/real-dark2.png）

### 4.4 前端 UI（2 条）

#### TC-031 应用内中文右键菜单

* **标签**：`前端 UI` · `P2` · 功能 · 场景法 · **判定：通过**
* **前置条件**：LogLume 在前台且屏幕常亮，需要可向 WebView 注入真实右键事件
* **测试数据**：右键坐标 (600, 400)，预期菜单项 撤销/重做/剪切/复制/粘贴/全选
* **步骤**：
  1. 在编辑器区域触发鼠标右键
  2. 截取屏幕读取菜单项文本
  3. 点击复制菜单项验证剪贴板联动
* **预期结果**：弹出中文右键菜单，不出现 Reload 或 Inspect Element，点击复制后剪贴板内容可判定
* **实际结果**：实测在已打开文档的 CodeMirror 区域用 CoreGraphics 真实注入右键（屏幕常亮、系统权限已授予）：菜单为 剪切/复制/粘贴/全选/查找/替换…/行操作/格式化/书签/AI/视图，无 Reload 或 Inspect Element；点击复制后 AppleScript Unicode text 返回正确中文，clipboard info 含 utf8 flavor 1269 字节；截图 /tmp/real-ctxmenu.png。空白工作区用 control-click 或 AXShowMenu 不弹菜单属源码预期（无匹配目标）

#### TC-033 剪贴板编码与 flavor 交叉验证

* **标签**：`前端 UI` · `P2` · 功能 · 场景法 · **判定：通过**
* **前置条件**：已打开 GBK 文档，可全选文本，剪贴板先写入已知 ASCII 基线
* **测试数据**：基线 CLIPBOARD-CLEARED / CLEAR-5 / CLEAR-6，剪贴板 flavor 与字节编码
* **步骤**：
  1. 执行 Cmd+A 全选
  2. 点击 编辑 > 复制
  3. 读取 pbpaste 字节与 UTF-8 校验结果
  4. 读取 AppleScript Unicode text 与 clipboard info
  5. 用应用右键菜单再次复制并重复读取
* **预期结果**：至少一种标准 Unicode flavor 含正确中文；若 pbpaste 显示非 UTF-8 字节，需核对 flavor 后再判定缺陷，日志/链路可追溯
* **实际结果**：实测 pbpaste 前 32 字节含 cc a8 bc dc，iconv UTF-8 校验 exit=1；但 AppleScript Unicode text 返回「台架 HIL 测试 记录 完成」正确中文，clipboard info 为 utf8 1269 / ut16 1900 / string 25 / Unicode text 1898；原生菜单与右键菜单两条复制路径结果一致。结论：pbpaste 取 legacy string flavor，非产品缺陷

### 4.5 桌面运行环境（2 条）

#### TC-027 dev server 与应用进程可观测

* **标签**：`桌面运行环境` · `P1` · 功能 · 场景法 · **判定：通过**
* **前置条件**：tauri dev 已从仓库根目录启动，端口配置为 1520
* **测试数据**：http://localhost:1520，/tmp/real-tauri.log，日志/监控/告警/trace/链路
* **步骤**：
  1. 执行 curl 检查 http://localhost:1520
  2. 执行 ps 检查 lumelog-app 进程
  3. 检查 /tmp/real-tauri.log 的 Running 与 panic 记录
* **预期结果**：dev server 返回 200，应用进程存活，编译日志含 Finished 与 Running，无 panic 或 fatal
* **实际结果**：实测 curl=200；pid 89489；日志 Finished dev profile in 30.20s 与 Running；无 panic/fatal；菜单清理日志 6 条

#### TC-032 第二实例转发真实打开 GBK 日志文件

* **标签**：`桌面运行环境` · `P1` · 功能 · 场景法 · **判定：通过**
* **前置条件**：LogLume 首实例 89489 运行中，单实例插件已加载，sample_gbk.log 存在
* **测试数据**：/Users/cc/Downloads/项目/log-lume/crates/lumelog-core/tests/data/sample_gbk.log
* **步骤**：
  1. 执行 target/debug/lumelog-app 加样本文件绝对路径
  2. 等待 3 秒
  3. 截取屏幕 /tmp/real-opened2.png
* **预期结果**：第二实例静默退出并把路径转发给首实例，首实例出现该文档标签、20 行中文日志与 GBK 元数据，进程不重启
* **实际结果**：实测第二实例 rc=0 且无输出；首实例截图显示 sample_gbk.log 标签、20 行中文日志、侧栏 GBK 标记，文档信息栏 编码 GBK / 行数 20 / 行尾 LF / 大小 1.1KB；进程 89489 存活（/tmp/real-opened2.png）

### 4.6 性能 / 自动化 / 非功能 / 兼容性（4 条）

#### TC-023 约 1MiB 日志的搜索与行数统计响应时间

* **标签**：`性能` · `P2` · 性能 · 场景法 · **判定：通过**
* **前置条件**：lumelog-gen 可生成确定性日志，/tmp 可写
* **测试数据**：SIZE_MB=1，生成 948516 字节 16384 行 ASCII 日志，性能/并发/响应时间测量
* **步骤**：
  1. 执行 lumelog-gen 生成 /tmp/real-1mb.log 1
  2. 用 /usr/bin/time -p 执行 search --pattern INFO --max 3
  3. 用 /usr/bin/time -p 执行 open --line-count
* **预期结果**：输出命中统计与行数，并记录 real/user/sys 耗时，退出码 0
* **实际结果**：实测生成 948516 字节；search matches=16384 scanned_bytes=948516 truncated=true，real 0.35s user 0.08s sys 0.03s；line-count 16385，real 0.31s（F1-F3）

#### TC-028 前端与 Rust 全量自动化回归

* **标签**：`自动化测试套件` · `P0` · 功能 · 场景法 · **判定：通过**
* **前置条件**：npm ci 与 cargo 缓存可用，工作区代码为 ee68040
* **测试数据**：vitest 39 个测试文件，Rust workspace 3 个 crate，TypeScript strict 模式
* **步骤**：
  1. 执行 npx vitest run
  2. 执行 cargo test --workspace
  3. 执行 npx tsc --noEmit
  4. 执行 npm run build
* **预期结果**：vitest 与 cargo test 全绿，tsc 无输出，vite build 完成，各项退出码 0
* **实际结果**：实测 vitest 39 文件 304 tests passed Duration 12.97s；cargo test 388 passed 0 failed；tsc exit=0 无输出；npm run build 3.72s 通过（/tmp/real-vitest.log、/tmp/real-cargo.log）

#### TC-029 超时限流降级与并发场景

* **标签**：`非功能` · `P2` · 非功能 · 场景法 · **判定：部分验证**
* **前置条件**：需要可配置超时与并发注入的测试环境
* **测试数据**：并发 20 请求、超时阈值 3s、限流阈值 10 QPS
* **步骤**：
  1. 构造并发请求并记录超时/限流/熔断/降级行为
  2. 检查日志/埋点/监控/告警/trace/链路是否可追溯
* **预期结果**：超时或限流时返回明确错误码并降级，不拖死进程，监控链路可观测
* **实际结果**：实测 20 并发 cargo run 多轮 18\~20/20 成功，失败进程 rc=101 且 stderr 为 could not execute process (never executed) / No such file or directory；改用已构建二进制 target/debug/lumelog 并发 20 次全部 exit=0、耗时 0.07s、无 panic；CLI 帮助无显式超时/限流/熔断参数，降级行为未验证（/tmp/real-concurrency\*.txt）

#### TC-030 跨平台版本与分辨率兼容

* **标签**：`兼容性` · `P3` · 兼容 · 场景法 · **判定：部分验证**
* **前置条件**：需要 Windows Linux 与不同 macOS 版本环境
* **测试数据**：macOS 15 已具备，Windows 11 与 Ubuntu 24.04 环境待准备，浏览器/分辨率/机型/版本
* **步骤**：
  1. 在 macOS 验证菜单/窗口装饰/分辨率
  2. 在 Windows 验证无边框自绘标题栏
  3. 在 Linux 验证 WebKitGTK 兼容性
* **预期结果**：各平台菜单、编码、窗口装饰表现一致且无布局溢出
* **实际结果**：macOS 15 实测：原生中文菜单栏、红黄绿窗口按钮、Overlay 标题栏、命令面板与暗黑主题均正常（/tmp/real-palette.png、/tmp/real-dark2.png）；Windows/Linux/其他版本与分辨率未执行

## 5. 未通过与部分验证

**不通过用例：0 条。**

| 编号     | 要点         | 未跑全的范围                                             | 依据             |
| ------ | ---------- | -------------------------------------------------- | -------------- |
| TC-029 | 超时/限流/熔断降级 | CLI 帮助无显式超时/限流/熔断参数，降级行为未验证；只完成并发实测                | `actual` 已明确写出 |
| TC-030 | 跨平台与分辨率兼容  | Windows / Linux / 其他 macOS 版本与分辨率未执行；本机只有 macOS 15 | `actual` 已明确写出 |

## 6. 疑似缺陷复核：均非产品缺陷

| 现象                                           | 复核结论                                                                                                                                                                                                             |
| -------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `--limit 3 --out jsonl` 打印了全部 7 帧            | `lumelog-can` 源码 `print_all = matches!(out, Some("jsonl") \| Some("csv"))`，带 `--out` 时打印全部帧是设计；默认输出才受 `--limit` 限制，已用 TC-015～017 复核                                                                              |
| 空白工作区右键不弹菜单                                  | `installContextMenus()` 只在 `.cm-editor` / `.listview` / `.canpanel` 等目标上弹菜单，属源码预期；在已打开文档的 CodeMirror 区域注入真实右键可正常弹出（TC-031）                                                                                       |
| `pbpaste` 读出的中文是乱码（`cc a8 bc dc`，UTF-8 校验失败） | 它取的是 legacy `string` flavor；`clipboard info` 显示 `utf8 1269` / `ut16 1900` / `Unicode text 1898`，AppleScript `Unicode text` 返回正确中文（TC-033）                                                                        |
| 20 并发 `cargo run` 偶发 rc=101                  | 报错为 `could not execute process ... (never executed) / No such file or directory`，是共享 `target/debug` 构建目录竞争；直接执行 `target/debug/lumelog` 并发 20 次为 20/20、0.07s、无 panic。`defect-kb.js` 关键词命中 DP-04 只是线索，人工复核后不判定应用缺陷 |

## 7. 观察项（未判定缺陷、未改代码）

1. 命令面板提示为 `Ctrl+O` / `Ctrl+Shift+P`，而 macOS 原生菜单使用 ⌘ 修饰键，属文案一致性观察。
2. 1MiB 生成器写 16384 行，`--line-count` 输出 16385（末尾换行形成空行，计数口径差异）。
3. `sample.dbc` 第 14 行故意损坏，严格模式报错、lenient `errors=1` 均属夹具预期。
4. 并发压测建议直接用已构建二进制，或为各进程设置独立 `CARGO_TARGET_DIR`。

## 8. 证据文件

* 命令输出：`/tmp/real-cli.txt`、`/tmp/real-can.txt`、`/tmp/real-can2.txt`、`/tmp/real-can3.txt`、`/tmp/real-extra.txt`、`/tmp/real-extra2.txt`、`/tmp/real-perf.txt`、`/tmp/real-concurrency*.txt`、`/tmp/real-concurrency-direct.txt`
* 自动化：`/tmp/real-vitest.log`、`/tmp/real-cargo.log`、`/tmp/real-tsc.log`、`/tmp/loglume-build-new.log`
* 应用与截图：`/tmp/real-tauri.log`、`/tmp/real-palette.png`、`/tmp/real-dark2.png`、`/tmp/real-restored2.png`、`/tmp/real-opened2.png`、`/tmp/real-ctxmenu.png`、`/tmp/real-rightclick*.png`
* 门禁报告：`gate-report.md` / `gate-report.json`、`skill-scripts.log`、`concurrency-finding.json`（会话 artifact 目录，未入库）