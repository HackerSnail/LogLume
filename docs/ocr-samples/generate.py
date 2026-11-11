"""Generate the OCR acceptance corpus: simulated table screenshots.

Dev-only fixture generator (spec §9). NOT run in CI — the Rust test
`ocr_samples.rs` reads the committed images/boxes/truth, so Python is
unnecessary after generation. Regenerate with:

    python docs/ocr-samples/generate.py

Each sample produces three files next to each other:

    <name>.png        the rendered table screenshot
    <name>.boxes.tsv  the "detector" output: one word per line with its pixel box
    <name>.truth.tsv  the expected grid: one row per line, cells tab separated

The boxes are the ideal ones (no detector noise) — the perturbation tests in
`lumelog-core::ocr` cover the dirty variants. The truth file is what the
column reconstruction must reproduce from the boxes.

Layouts follow spec §9.1: CAN bench tool, SCADA/HMI alarm list, monitoring
panel, terminal/database result table.
"""
import os

from PIL import Image, ImageDraw, ImageFilter, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))

FONT_CJK = "C:/Windows/Fonts/msyh.ttc"
FONT_MONO = "C:/Windows/Fonts/consola.ttf"

# Fixed data — no randomness, so the corpus is reproducible.
CAN_ROWS = [
    ("0x01", "MotorSpeed", "8", "00 1A 2B 3C 00 00 00 00", "10"),
    ("0x02", "MotorTorque", "8", "00 00 00 64 00 00 00 00", "10"),
    ("0x03", "BatteryVoltage", "2", "0C E4", "100"),
    ("0x04", "CoolantTemp", "2", "00 5A", "1000"),
    ("0x11", "BrakePressure", "4", "01 C8 00 00", "20"),
    ("0x12", "GearPosition", "1", "03", "50"),
    ("0x21", "AmbientTemp", "2", "00 1E", "2000"),
    ("0x22", "EngineRpm", "2", "0B B8", "10"),
]

ALARM_ROWS = [
    ("10:23:41", "T-1042", "冷却水温高", "高", "报警"),
    ("10:23:47", "T-2210", "液压压力低", "中", "报警"),
    ("10:24:02", "T-1042", "冷却水温高", "高", "报警"),
    ("10:24:15", "M-3301", "主电机未启动", "高", "故障"),
    ("10:24:31", "T-5508", "电池电压偏低", "低", "报警"),
    ("10:25:03", "T-2210", "液压压力低", "中", "报警"),
]

PANEL_ROWS = [
    ("10:31:02", "rig-01", "motor.speed", "1180 rpm", "正常"),
    ("10:31:02", "rig-01", "motor.torque", "336 Nm", "正常"),
    ("10:31:05", "rig-02", "battery.voltage", "12.42 V", "正常"),
    ("10:31:05", "rig-02", "coolant.temp", "92.1 C", "预警"),
    ("10:31:09", "rig-03", "line.throughput", "820 /min", "正常"),
    ("10:31:09", "rig-03", "scrap.rate", "1.8 %", "预警"),
]

TERMINAL_ROWS = [
    ("lume-core-7d9f", "1/1", "Running", "0", "12d"),
    ("lume-web-4b2a", "1/1", "Running", "0", "12d"),
    ("lume-ocr-worker", "0/1", "Error", "7", "3m14s"),
    ("lume-postgres", "1/1", "Running", "0", "31d"),
    ("lume-redis", "1/1", "Running", "0", "31d"),
    ("lume-grafana", "1/1", "Running", "1", "45d"),
]


def render(title, headers, rows, font_path, font_size, col_gap, pad=14, header_h=34,
           row_h=26, bg=(255, 255, 255), header_bg=(238, 240, 244), fg=(20, 22, 28),
           header_fg=(10, 12, 16), grid=True, grid_color=(226, 228, 232)):
    """Render a table and return (image, boxes, truth).

    `boxes` is the ideal detector output in image pixels: one entry per drawn
    word with its ink box. `truth` is the grid the reconstruction must build.
    """
    font = ImageFont.truetype(font_path, font_size)
    title_font = ImageFont.truetype(font_path, font_size + 3)
    space_gap = max(3, font_size // 3)

    # A CJK phrase is one "word"; latin text splits on spaces.
    def cell_words(text):
        if any(ord(c) > 0x2E80 for c in text):
            return [text]
        return text.split(" ") if text else []

    def cell_width(text):
        total = 0
        for i, word in enumerate(cell_words(text)):
            bbox = font.getbbox(word)
            total += bbox[2] - bbox[0]
            if i:
                total += space_gap
        return total

    # Column x positions: the widest cell of the column, then the gap.
    widths = []
    for ci in range(len(headers)):
        widest = 0
        for text in [headers[ci]] + [r[ci] for r in rows]:
            widest = max(widest, cell_width(text))
        widths.append(widest)

    xs = []
    x = pad
    for w in widths:
        xs.append(x)
        x += w + col_gap

    title_bbox = title_font.getbbox(title)
    title_h = title_bbox[3] - title_bbox[1]
    table_top = pad + title_h + 12
    height = table_top + header_h + row_h * len(rows) + pad
    width = x + pad
    img = Image.new("RGB", (width, height), bg)
    draw = ImageDraw.Draw(img)

    draw.text((pad, pad - title_bbox[1]), title, font=title_font, fill=fg)

    draw.rectangle([0, table_top, width, table_top + header_h], fill=header_bg)
    if grid:
        for r in range(len(rows) + 1):
            y = table_top + header_h + r * row_h
            draw.line([(0, y), (width, y)], fill=grid_color)

    boxes = []
    truth = []

    def draw_row(cells, y, is_header):
        row_truth = []
        for ci, text in enumerate(cells):
            words = cell_words(text)
            row_truth.append(text)
            cx = xs[ci]
            for word in words:
                bbox = font.getbbox(word)
                draw.text((cx, y), word, font=font, fill=header_fg if is_header else fg)
                boxes.append((word, cx + bbox[0], y + bbox[1], bbox[2] - bbox[0], bbox[3] - bbox[1]))
                cx += (bbox[2] - bbox[0]) + space_gap
        return row_truth

    header_text_h = font.getbbox("Ag")[3] - font.getbbox("Ag")[1]
    truth.append(draw_row(headers, table_top + (header_h - header_text_h) // 2, True))
    for i, row in enumerate(rows):
        y = table_top + header_h + i * row_h + (row_h - header_text_h) // 2
        truth.append(draw_row(list(row), y, False))

    return img, boxes, truth


def degrade(img, scale=0.78, jpeg_quality=55, blur=0.6, bg=(247, 247, 250)):
    """Make a noisy variant: jpeg artifacts, blur, downscale, lighter paper."""
    import io

    buf = io.BytesIO()
    img.save(buf, format="JPEG", quality=jpeg_quality)
    buf.seek(0)
    out = Image.open(buf).convert("RGB")
    if blur:
        out = out.filter(ImageFilter.GaussianBlur(blur))
    w = max(1, int(out.width * scale))
    h = max(1, int(out.height * scale))
    out = out.resize((w, h), Image.LANCZOS)
    canvas = Image.new("RGB", (out.width + 12, out.height + 12), bg)
    canvas.paste(out, (6, 6))
    return canvas, scale, 6, 6


def write_sample(name, img, boxes, truth):
    png = os.path.join(HERE, name + ".png")
    img.save(png, optimize=True)
    with open(os.path.join(HERE, name + ".boxes.tsv"), "w", encoding="utf-8", newline="\n") as f:
        for text, x, y, w, h in boxes:
            f.write(f"{text}\t{x}\t{y}\t{w}\t{h}\n")
    with open(os.path.join(HERE, name + ".truth.tsv"), "w", encoding="utf-8", newline="\n") as f:
        for row in truth:
            f.write("\t".join(row) + "\n")
    return os.path.getsize(png)


def scale_boxes(boxes, s, ox, oy):
    return [(t, round(ox + x * s), round(oy + y * s), round(w * s), round(h * s)) for (t, x, y, w, h) in boxes]


def main():
    os.makedirs(HERE, exist_ok=True)
    written = []

    # --- CAN bench tool (PCAN-View / CANoe like) ---
    img, boxes, truth = render(
        "PCAN-View — 报文列表",
        ["ID", "名称", "DLC", "数据", "周期(ms)"],
        CAN_ROWS,
        FONT_CJK,
        16,
        col_gap=42,
    )
    written.append(("can-tool-clean", write_sample("can-tool-clean", img, boxes, truth)))
    img2, s, ox, oy = degrade(img)
    written.append(("can-tool-noisy", write_sample("can-tool-noisy", img2, scale_boxes(boxes, s, ox, oy), truth)))

    # --- SCADA / HMI alarm list ---
    img, boxes, truth = render(
        "报警列表 — 工位 3",
        ["时间", "位号", "描述", "优先级", "状态"],
        ALARM_ROWS,
        FONT_CJK,
        16,
        col_gap=46,
    )
    written.append(("scada-alarm-clean", write_sample("scada-alarm-clean", img, boxes, truth)))
    img2, s, ox, oy = degrade(img, scale=0.72, jpeg_quality=45)
    written.append(("scada-alarm-noisy", write_sample("scada-alarm-noisy", img2, scale_boxes(boxes, s, ox, oy), truth)))

    # --- monitoring panel (Grafana / Zabbix like) ---
    img, boxes, truth = render(
        "监控面板 — 产线指标",
        ["时间", "主机", "指标", "数值", "状态"],
        PANEL_ROWS,
        FONT_CJK,
        15,
        col_gap=40,
    )
    written.append(("monitor-panel-clean", write_sample("monitor-panel-clean", img, boxes, truth)))
    img2, s, ox, oy = degrade(img, scale=0.75)
    written.append(("monitor-panel-noisy", write_sample("monitor-panel-noisy", img2, scale_boxes(boxes, s, ox, oy), truth)))

    # --- terminal / database result table (monospace) ---
    img, boxes, truth = render(
        "$ kubectl get pods",
        ["NAME", "READY", "STATUS", "RESTARTS", "AGE"],
        TERMINAL_ROWS,
        FONT_MONO,
        16,
        col_gap=38,
        bg=(28, 30, 34),
        header_bg=(44, 47, 53),
        fg=(222, 226, 232),
        header_fg=(240, 243, 248),
        grid_color=(52, 56, 62),
    )
    written.append(("terminal-table-clean", write_sample("terminal-table-clean", img, boxes, truth)))
    img2, s, ox, oy = degrade(img, scale=0.8, jpeg_quality=60, bg=(28, 30, 34))
    written.append(("terminal-table-noisy", write_sample("terminal-table-noisy", img2, scale_boxes(boxes, s, ox, oy), truth)))

    total = 0
    for name, size in written:
        total += size
        print(f"{name}: {size // 1024} KiB")
    print(f"total images: {total // 1024} KiB")


if __name__ == "__main__":
    main()
