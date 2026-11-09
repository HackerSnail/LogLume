// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { LanguageSupport } from "@codemirror/language";
import {
  AVAILABLE_MODES,
  MODE_LABELS,
  autoModeId,
  autoModeIdForPath,
  modeIdForExt,
  modeIdForFile,
  pluginModeId,
  registerPluginLanguages,
  resolveLanguage,
  syntaxModes,
  type ModeChoice,
} from "./language";
import type { SyntaxPlugin } from "./ipc";

/** True when the language's parser reports a syntax error node for `src`. */
function hasError(lang: LanguageSupport, src: string): boolean {
  let bad = false;
  lang.language.parser.parse(src).iterate({
    enter: (node) => {
      if (node.type.isError) bad = true;
    },
  });
  return bad;
}

describe("modeIdForExt", () => {
  it("maps extensions to mode ids", () => {
    const cases: Array<[string, ModeChoice | null]> = [
      ["py", "python"],
      ["PY", "python"],
      ["pyw", "python"],
      ["ts", "typescript"],
      ["tsx", "tsx"],
      ["mts", "typescript"],
      ["cts", "typescript"],
      ["js", "javascript"],
      ["mjs", "javascript"],
      ["jsx", "jsx"],
      ["yml", "yaml"],
      ["yaml", "yaml"],
      ["toml", "toml"],
      ["sh", "shell"],
      ["bash", "shell"],
      ["diff", "diff"],
      ["patch", "diff"],
      ["ps1", "powershell"],
      ["psd1", "powershell"],
      ["rs", "rust"],
      ["java", "java"],
      ["md", "markdown"],
      ["sql", "sql"],
      ["html", "html"],
      ["htm", "html"],
      ["css", "css"],
      ["xml", "xml"],
      ["svg", "xml"],
      ["plist", "xml"],
      ["log", "log"],
      ["ini", "ini"],
      ["conf", "ini"],
      ["json", "json"],
      ["c", "c"],
      ["h", "c"],
      ["hpp", "c"],
      ["cxx", "c"],
      ["cc", "c"],
      ["go", "go"],
      ["cs", "csharp"],
      ["kt", "kotlin"],
      ["kts", "kotlin"],
      ["scala", "scala"],
      ["sc", "scala"],
      ["dart", "dart"],
      ["m", "objectivec"],
      ["mm", "objectivec"],
      ["swift", "swift"],
      ["rb", "ruby"],
      ["lua", "lua"],
      ["pl", "perl"],
      ["pm", "perl"],
      ["hs", "haskell"],
      ["erl", "erlang"],
      ["hrl", "erlang"],
      ["r", "r"],
      ["clj", "clojure"],
      ["cljs", "clojure"],
      ["proto", "protobuf"],
      ["cmake", "cmake"],
      ["jsonc", "json"],
      ["json5", "json"],
      ["php", "php"],
      ["php5", "php"],
      ["phtml", "php"],
      ["vue", "vue"],
      ["svelte", "svelte"],
      ["scss", "scss"],
      ["sass", "sass"],
      ["less", "less"],
      ["v", "verilog"],
      ["sv", "verilog"],
      ["svh", "verilog"],
      ["vh", "verilog"],
      ["vhd", "vhdl"],
      ["vhdl", "vhdl"],
      ["graphql", "graphql"],
      ["gql", "graphql"],
      ["sol", "solidity"],
      ["dot", "dot"],
      ["gv", "dot"],
      ["liquid", "liquid"],
      ["wat", "wast"],
      ["wast", "wast"],
      ["dockerfile", "dockerfile"],
      ["s", "gas"],
      ["asm", "gas"],
      ["ml", "ocaml"],
      ["mli", "ocaml"],
      ["fs", "fsharp"],
      ["fsx", "fsharp"],
      ["sml", "sml"],
      ["coffee", "coffeescript"],
      ["cr", "crystal"],
      ["d", "d"],
      ["elm", "elm"],
      ["f", "fortran"],
      ["f90", "fortran"],
      ["groovy", "groovy"],
      ["gradle", "groovy"],
      ["jl", "julia"],
      ["pas", "pascal"],
      ["pp", "puppet"],
      ["pug", "pug"],
      ["scm", "scheme"],
      ["tcl", "tcl"],
      ["vb", "vb"],
      ["vbs", "vb"],
      ["tex", "latex"],
      ["nginx", "nginx"],
      ["webidl", "webidl"],
      ["properties", "properties"],
    ];
    for (const [ext, mode] of cases) expect(modeIdForExt(ext), ext).toBe(mode);
  });

  it("falls back to plain text for unknown or missing extensions", () => {
    expect(modeIdForExt("txt")).toBeNull();
    expect(modeIdForExt("rsx")).toBeNull();
    expect(modeIdForExt("")).toBeNull();
  });
});

describe("modeIdForFile / autoModeIdForPath", () => {
  it("recognises extension-less file names", () => {
    expect(modeIdForFile("C:\\repo\\Dockerfile")).toBe("dockerfile");
    expect(modeIdForFile("/repo/Dockerfile.dev")).toBe("dockerfile");
    expect(modeIdForFile("CMakeLists.txt")).toBe("cmake");
    expect(modeIdForFile("nginx.conf")).toBe("nginx");
    expect(modeIdForFile("Jenkinsfile")).toBe("groovy");
    expect(modeIdForFile("Gemfile")).toBe("ruby");
    expect(modeIdForFile("main.rs")).toBeNull();
  });

  it("prefers whole-file names, then the extension", () => {
    expect(autoModeIdForPath("/repo/Dockerfile")).toBe("dockerfile");
    expect(autoModeIdForPath("/repo/Dockerfile.dev")).toBe("dockerfile");
    expect(autoModeIdForPath("C:\\repo\\src\\main.rs")).toBe("rust");
    expect(autoModeIdForPath("CMakeLists.txt")).toBe("cmake");
    expect(autoModeIdForPath("main.unknown")).toBeNull();
  });
});

describe("resolveLanguage", () => {
  it("resolves every selectable mode (registry drift guard)", async () => {
    for (const mode of AVAILABLE_MODES) {
      if (mode === "auto" || mode === "plain") continue;
      expect(await resolveLanguage(mode), mode).toBeInstanceOf(LanguageSupport);
    }
  });

  it("returns PlainText for auto, plain and null", async () => {
    expect(await resolveLanguage(null)).toBeNull();
    expect(await resolveLanguage("auto")).toBeNull();
    expect(await resolveLanguage("plain")).toBeNull();
  });

  it("loads a language once", async () => {
    const first = await resolveLanguage("python");
    expect(await resolveLanguage("python")).toBe(first);
  });

  it("gives the TypeScript mode a TS dialect", async () => {
    const ts = await resolveLanguage("typescript");
    const js = await resolveLanguage("javascript");
    const src = "interface Foo { a: number }";
    expect(hasError(ts!, src)).toBe(false);
    expect(hasError(js!, src)).toBe(true);
  });

  it("uses JSX/TSX dialects for .jsx/.tsx and plain TS for .ts", async () => {
    const js = await resolveLanguage("javascript");
    const jsx = await resolveLanguage("jsx");
    const ts = await resolveLanguage("typescript");
    const tsx = await resolveLanguage("tsx");

    const jsxSrc = 'const x = <div className="a" />;';
    expect(hasError(jsx!, jsxSrc)).toBe(false);
    expect(hasError(js!, jsxSrc)).toBe(true);

    const tsxSrc = "const x: JSX.Element = <div />;";
    expect(hasError(tsx!, tsxSrc)).toBe(false);

    const generic = "function id<T>(x: T): T { return x; }";
    expect(hasError(ts!, generic)).toBe(false);
  });

  it("loads and parses samples in the added common languages", async () => {
    const samples: Array<[ModeChoice, string]> = [
      ["go", 'package main\nfunc main() { println("hi") }'],
      ["csharp", "class A { static void Main() {} }"],
      ["kotlin", 'fun main() { println("hi") }'],
      ["scala", 'object A { def main(args: Array[String]) = println("hi") }'],
      ["dart", "void main() { print('hi'); }"],
      ["objectivec", "#import <Foundation/Foundation.h>\nint main() { return 0; }"],
      ["swift", 'print("hi")'],
      ["ruby", "def hi\n  puts 'hi'\nend"],
      ["lua", "print('hi')"],
      ["perl", 'print "hi\\n";'],
      ["haskell", 'main = putStrLn "hi"'],
      ["erlang", "-module(a).\n-export([f/0]).\nf() -> ok."],
      ["r", "x <- c(1, 2)\nprint(x)"],
      ["clojure", '(println "hi")'],
      ["protobuf", 'syntax = "proto3";\nmessage A { int32 id = 1; }'],
      ["cmake", "cmake_minimum_required(VERSION 3.20)\nproject(a)"],
      ["php", '<?php echo "hi"; ?>'],
      ["vue", "<template><div>hi</div></template>"],
      ["svelte", "<h1>Hello</h1>"],
      ["scss", "$c: red;\n.a { color: $c; }"],
      ["sass", ".a\n  color: red"],
      ["less", "@c: red;\n.a { color: @c; }"],
      ["verilog", "module top(input a, output b);\nassign b = a;\nendmodule"],
      ["vhdl", "entity top is\nend entity;\narchitecture rtl of top is\nbegin\nend architecture;"],
      ["graphql", "query Foo { user { id name } }"],
      ["solidity", "pragma solidity ^0.8.0;\ncontract A { uint x; }"],
      ["dot", "digraph G {\n  a -> b;\n}"],
      ["liquid", "{% if user %}{{ user.name }}{% endif %}"],
      ["angular", '<div *ngIf="ok">{{ value }}</div>'],
      ["wast", '(module (func (export "f")))'],
      ["dockerfile", "FROM alpine\nRUN echo hi"],
      ["gas", "mov $1, %eax"],
      ["ocaml", "let x = 1"],
      ["fsharp", "let x = 1"],
      ["sml", "val x = 1"],
      ["coffeescript", "x = 1"],
      ["crystal", 'puts "hi"'],
      ["d", "void main() {}"],
      ["elm", "module Main exposing (..)"],
      ["fortran", "program hello\nend program"],
      ["groovy", 'println "hi"'],
      ["julia", 'println("hi")'],
      ["pascal", "program Hello; begin end."],
      ["puppet", 'node default { notify { "hi": } }'],
      ["pug", "div Hello"],
      ["scheme", '(display "hi")'],
      ["tcl", 'puts "hi"'],
      ["vb", "Module A\nEnd Module"],
      ["latex", "\\documentclass{article}"],
      ["nginx", "server { listen 80; }"],
      ["webidl", "interface A { attribute long x; };"],
      ["properties", "key=value"],
    ];
    for (const [mode, src] of samples) {
      const lang = await resolveLanguage(mode);
      expect(lang, mode).toBeInstanceOf(LanguageSupport);
      expect(hasError(lang!, src), mode).toBe(false);
    }
  });
});

describe("MODE_LABELS", () => {
  it("covers every mode with a non-empty display name", () => {
    for (const mode of AVAILABLE_MODES) {
      expect(MODE_LABELS[mode], mode).toBeTruthy();
    }
  });
});

describe("syntax plugins (plugin interface, increment 1)", () => {
  function plugin(overrides: Partial<SyntaxPlugin> = {}): SyntaxPlugin {
    return {
      id: "plc-log",
      name: "PLC 日志",
      extensions: ["plc", "plclog"],
      keywords: ["START", "STOP"],
      lineComment: "#",
      rules: [{ regex: "^\\d{2}:\\d{2}", tag: "className" }],
      ...overrides,
    };
  }

  it("resolves registered plugins and keeps builtin priority on AUTO", async () => {
    const warnings = registerPluginLanguages([plugin()]);
    expect(warnings).toEqual([]);

    const support = await resolveLanguage(pluginModeId("plc-log"));
    expect(support).toBeInstanceOf(LanguageSupport);
    expect(() => support!.language.parser.parse("12:00 START x")).not.toThrow();

    expect(autoModeId("plc")).toBe("plugin:plc-log");
    expect(autoModeId("PLCLOG")).toBe("plugin:plc-log");
    expect(autoModeId("py")).toBe("python"); // builtins are never hijacked
    expect(await resolveLanguage(pluginModeId("nope"))).toBeNull();
  });

  it("drops rules whose regex does not compile, with a warning", () => {
    const warnings = registerPluginLanguages([
      plugin({
        id: "bad",
        name: "坏规则",
        rules: [{ regex: "([unclosed", tag: "number" }],
      }),
    ]);
    expect(warnings).toHaveLength(1);
    expect(warnings[0]).toContain("坏规则");
    expect(syntaxModes().some((m) => m.id === "plugin:bad")).toBe(true); // mode survives
  });

  it("lists builtin modes first, then plugin modes", () => {
    registerPluginLanguages([plugin(), plugin({ id: "second", name: "第二" })]);
    const modes = syntaxModes();
    expect(modes.slice(0, AVAILABLE_MODES.length).map((m) => m.id)).toEqual([
      ...AVAILABLE_MODES,
    ]);
    expect(modes.slice(-2).map((m) => m.label)).toEqual(["PLC 日志", "第二"]);
    registerPluginLanguages([]); // reload with none → plugins disappear
    expect(syntaxModes()).toHaveLength(AVAILABLE_MODES.length);
  });
});
