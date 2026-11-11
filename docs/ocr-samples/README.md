# OCR 仿真样本集

列还原（`lumelog-core::ocr`）的回归语料。CI 断言：
`cargo test -p lumelog-core --test ocr_samples` → 逐格与逐列比对真值。

每样本三件套：`<name>.png`（图像）、`<name>.boxes.tsv`（逐词框
`text\tx\ty\tw\th`）、`<name>.truth.tsv`（期望网格，逐行 tab 分隔）。

## 样本种类

| 名称 | 来源 | 说明 |
|---|---|---|
| `can-tool-*` | 合成（CAN 台架工具报文列表） | clean / noisy（JPEG 压缩 + 模糊 + 缩小 + 浅底） |
| `scada-alarm-*` | 合成（SCADA/HMI 报警表） | clean / noisy |
| `monitor-panel-*` | 合成（Grafana/Zabbix 风格监控面板） | clean / noisy |
| `terminal-table-*` | 合成（kubectl get pods 风格终端表格） | clean / noisy |
| `can-tool-ocrwin` | **真实 OCR 输出**（Windows.Media.Ocr 跑 can-tool-clean.png） | 真实词框，含引擎噪声 |

`can-tool-ocrwin` 没有配图——它的词框就是识别引擎的真实产物，用来锁住真实噪声
下的列还原行为（标题行、逐字中文、漏识别的周期列）。

## 已知限制（真值里如实记录）

- **引擎切词**：Windows OCR 把 `1A` 切成 `1` + `A`（间隙 3px），与真实词间距在几何
  上无法区分，所以真值记的是 `00 1 A 2B 3C …`。列类型校验（spec §5.3）是兜底。
- **逐字中文**：中文被切成单字框，格内按间隙拼接（≤2px 视为同一个词，不插空格）。
- **识别错误**：真值保留了引擎的误识（`BatteryV01tage`、`Brakepressure`）——本语料
  验的是列还原，不是识别质量；识别质量由 §9.4 的门槛单独度量。

## 重新生成

    python docs/ocr-samples/generate.py
