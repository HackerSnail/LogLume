import { describe, expect, it } from "vitest";
import {
  ENCODINGS,
  ENCODING_LABELS,
  EOL_OPTIONS,
  extOf,
} from "./encoding-menu";

// Pure data/extension inference — no DOM, no Tauri mocks (spec §5, §4.4).

describe("extOf", () => {
  it("lowercases the extension for POSIX and Windows paths", () => {
    expect(extOf("/var/log/APP.LOG")).toBe("log");
    expect(extOf("C:\\logs\\APP.LOG")).toBe("log");
    expect(extOf("notes.Md")).toBe("md");
  });

  it("takes only the last extension of a dotted name", () => {
    expect(extOf("archive.tar.gz")).toBe("gz");
    expect(extOf("/tmp/a.b.CSV")).toBe("csv");
  });

  it("returns an empty string for dotfiles, extension-less names and directories", () => {
    expect(extOf(".gitignore")).toBe("");
    expect(extOf("README")).toBe("");
    expect(extOf("/var/log/")).toBe("");
    expect(extOf("")).toBe("");
  });
});

describe("encoding menu data", () => {
  it("keeps the 13 spec labels in order and derived labels in sync", () => {
    expect(ENCODINGS).toHaveLength(13);
    expect(ENCODING_LABELS).toEqual([
      "UTF-8",
      "UTF-8 BOM",
      "UTF-16 LE",
      "UTF-16 BE",
      "GBK",
      "GB18030",
      "BIG5",
      "Shift-JIS",
      "ISO-8859-1",
      "EUC-JP",
      "EUC-KR",
      "KOI8-R",
      "KOI8-U",
    ]);
    expect(ENCODING_LABELS).toEqual(ENCODINGS.map((e) => e.label));
    expect(new Set(ENCODING_LABELS).size).toBe(ENCODING_LABELS.length);
  });

  it("uses the wire label verbatim as dropdown text", () => {
    for (const enc of ENCODINGS) {
      expect(enc.display).toBe(enc.label);
    }
  });

  it("decodes GB2312 as GBK without listing it separately", () => {
    expect(ENCODING_LABELS).not.toContain("GB2312");
    expect(ENCODING_LABELS).toContain("GBK");
    expect(ENCODING_LABELS).toContain("GB18030"); // detection fallback
  });

  it("exposes exactly the three line-ending options", () => {
    expect(EOL_OPTIONS).toEqual(["LF", "CRLF", "MIXED"]);
    expect(new Set(EOL_OPTIONS).size).toBe(EOL_OPTIONS.length);
  });
});
