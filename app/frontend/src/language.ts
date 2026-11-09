//! Syntax highlighting modes (spec §8). Every language lives in one registry:
//! `mode id -> { extensions, loader }`. `modeIdForExt` answers "which mode does
//! this file use" without loading anything (it backs the AUTO choice); the
//! language packages themselves are pulled in on demand by `resolveLanguage`,
//! so a file that never opens a Python file never pays for the Python grammar.
//!
//! Highlight colours: `log`/`ini` carry their own inline `streamHighlight`;
//! everything else only emits tags and is coloured by `defaultHighlightStyle`
//! (mounted with `fallback: true` in `editor.ts`), which is why adding a
//! language needs no new `HighlightStyle`.

import {
  HighlightStyle,
  StreamLanguage,
  syntaxHighlighting,
  LanguageSupport,
  type StreamParser,
  type StringStream,
} from "@codemirror/language";
import { tags as t } from "@lezer/highlight";
import type { SyntaxPlugin } from "./ipc";

/** Log-style highlighting: timestamps, levels, numbers, hex, strings. */
const logLanguage = StreamLanguage.define<{ inQuotes: boolean }>({
  startState: () => ({ inQuotes: false }),
  copyState: (s) => ({ inQuotes: s.inQuotes }),
  token(stream, state) {
    // Timestamp prefix only — consume just the timestamp, not the whole line,
    // so the level / numbers later on the same line still get coloured.
    if (
      stream.sol() &&
      stream.match(/^\[?\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}(?::\d{2})?(?:\.\d+)?\]?/)
    ) {
      return "className";
    }
    if (stream.eatSpace()) return null;
    if (state.inQuotes) {
      if (stream.match(/^[^"]*"?/)) {
        if (stream.string[stream.pos - 1] === '"') state.inQuotes = false;
      } else {
        stream.next();
      }
      return "string";
    }
    if (stream.match(/^"([^"]|"")*"/)) return "string";
    if (stream.eat('"')) {
      state.inQuotes = true;
      return "string";
    }
    if (stream.match(/^0[xX][0-9a-fA-F]+/)) return "number";
    if (stream.match(/^\w+/)) {
      const word = stream.current().toUpperCase();
      if (/^(TRACE|DEBUG|INFO|NOTICE|WARN|WARNING|ERROR|ERR|FATAL|CRITICAL)$/.test(word)) {
        return "keyword";
      }
      return null;
    }
    stream.next();
    return null;
  },
});

/** Minimal INI: `[sections]`, `key = value`, `#`/`;` comments. */
const iniLanguage = StreamLanguage.define({
  token(stream) {
    if (stream.sol()) {
      if (/^[#;]/.test(stream.string.slice(stream.pos))) {
        stream.skipToEnd();
        return "comment";
      }
      if (stream.match(/^\[[^\]]*\]/)) return "className";
      if (stream.match(/^[^=\s]+(?=\s*=)/)) return "propertyName";
    }
    if (stream.eatSpace()) return null;
    if (stream.eat("=")) return "operator";
    if (/[#;]/.test(stream.string[stream.pos])) {
      stream.skipToEnd();
      return "comment";
    }
    if (stream.match(/^[^#;\s]+/)) return "string";
    stream.next();
    return null;
  },
});

/** Inline highlight style for the log / INI stream modes (no CSS needed). */
const streamHighlight = HighlightStyle.define([
  { tag: t.keyword, color: "#b25000", fontWeight: "bold" }, // log level
  { tag: t.className, color: "#708090" }, // timestamp / INI [section]
  { tag: t.number, color: "#1a6fb0" }, // number / hex
  { tag: t.string, color: "#0a7d33" }, // value / quoted string
  { tag: t.comment, color: "#8a8a8a", fontStyle: "italic" },
  { tag: t.propertyName, color: "#800080" }, // INI key
  { tag: t.operator, color: "#666" },
]);

/** The set of languages the status-bar switcher offers. */
export const AVAILABLE_MODES = [
  "auto",
  "plain",
  "log",
  "ini",
  "json",
  "c",
  "javascript",
  "jsx",
  "typescript",
  "tsx",
  "python",
  "php",
  "html",
  "vue",
  "svelte",
  "css",
  "scss",
  "sass",
  "less",
  "xml",
  "markdown",
  "sql",
  "rust",
  "java",
  "go",
  "csharp",
  "kotlin",
  "scala",
  "dart",
  "objectivec",
  "swift",
  "ruby",
  "lua",
  "perl",
  "haskell",
  "erlang",
  "r",
  "clojure",
  "protobuf",
  "cmake",
  "verilog",
  "vhdl",
  "graphql",
  "solidity",
  "dot",
  "gdscript",
  "godot",
  "liquid",
  "angular",
  "wast",
  "dockerfile",
  "gas",
  "ocaml",
  "fsharp",
  "sml",
  "coffeescript",
  "crystal",
  "d",
  "elm",
  "fortran",
  "groovy",
  "julia",
  "pascal",
  "puppet",
  "pug",
  "scheme",
  "tcl",
  "vb",
  "latex",
  "nginx",
  "webidl",
  "properties",
  "astro",
  "hcl",
  "zig",
  "yaml",
  "toml",
  "shell",
  "diff",
  "powershell",
] as const;
export type ModeChoice = (typeof AVAILABLE_MODES)[number];
/** Everything `resolveLanguage` accepts: built-ins plus `plugin:<id>`. */
export type ModeId = ModeChoice | `plugin:${string}`;

/** Dropdown text — mode ids stay stable, these are only what the user reads. */
export const MODE_LABELS: Record<ModeChoice, string> = {
  auto: "Auto",
  plain: "Plain Text",
  log: "Log",
  ini: "INI",
  json: "JSON",
  c: "C/C++",
  javascript: "JavaScript",
  jsx: "JSX",
  typescript: "TypeScript",
  tsx: "TSX",
  python: "Python",
  php: "PHP",
  html: "HTML",
  vue: "Vue",
  svelte: "Svelte",
  css: "CSS",
  scss: "SCSS",
  sass: "Sass",
  less: "Less",
  xml: "XML",
  markdown: "Markdown",
  sql: "SQL",
  rust: "Rust",
  java: "Java",
  go: "Go",
  csharp: "C#",
  kotlin: "Kotlin",
  scala: "Scala",
  dart: "Dart",
  objectivec: "Objective-C",
  swift: "Swift",
  ruby: "Ruby",
  lua: "Lua",
  perl: "Perl",
  haskell: "Haskell",
  erlang: "Erlang",
  r: "R",
  clojure: "Clojure",
  protobuf: "Protobuf",
  cmake: "CMake",
  verilog: "Verilog",
  vhdl: "VHDL",
  graphql: "GraphQL",
  solidity: "Solidity",
  dot: "Graphviz DOT",
  gdscript: "GDScript",
  godot: "Godot Resource",
  liquid: "Liquid",
  angular: "Angular",
  wast: "WebAssembly",
  dockerfile: "Dockerfile",
  gas: "GNU Assembly",
  ocaml: "OCaml",
  fsharp: "F#",
  sml: "Standard ML",
  coffeescript: "CoffeeScript",
  crystal: "Crystal",
  d: "D",
  elm: "Elm",
  fortran: "Fortran",
  groovy: "Groovy",
  julia: "Julia",
  pascal: "Pascal",
  puppet: "Puppet",
  pug: "Pug",
  scheme: "Scheme",
  tcl: "Tcl",
  vb: "Visual Basic",
  latex: "LaTeX",
  nginx: "Nginx",
  webidl: "WebIDL",
  properties: "Properties",
  astro: "Astro",
  hcl: "HCL / Terraform",
  zig: "Zig",
  yaml: "YAML",
  toml: "TOML",
  shell: "Shell",
  diff: "Diff",
  powershell: "PowerShell",
};

/** Extension (no dot, lowercased) → mode id. Absent = PlainText. */
const EXT_MODES: Record<string, ModeChoice> = {
  log: "log",
  ini: "ini",
  cfg: "ini",
  conf: "ini",
  json: "json",
  c: "c",
  h: "c",
  cpp: "c",
  cc: "c",
  cxx: "c",
  hpp: "c",
  hh: "c",
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  jsx: "jsx",
  ts: "typescript",
  tsx: "tsx",
  mts: "typescript",
  cts: "typescript",
  py: "python",
  pyw: "python",
  pyi: "python",
  php: "php",
  php3: "php",
  php4: "php",
  php5: "php",
  phtml: "php",
  html: "html",
  htm: "html",
  vue: "vue",
  svelte: "svelte",
  css: "css",
  scss: "scss",
  sass: "sass",
  less: "less",
  xml: "xml",
  xsd: "xml",
  xsl: "xml",
  svg: "xml",
  plist: "xml",
  resx: "xml",
  md: "markdown",
  markdown: "markdown",
  sql: "sql",
  rs: "rust",
  java: "java",
  go: "go",
  cs: "csharp",
  kt: "kotlin",
  kts: "kotlin",
  scala: "scala",
  sc: "scala",
  dart: "dart",
  m: "objectivec",
  mm: "objectivec",
  swift: "swift",
  rb: "ruby",
  rake: "ruby",
  gemspec: "ruby",
  lua: "lua",
  pl: "perl",
  pm: "perl",
  hs: "haskell",
  lhs: "haskell",
  erl: "erlang",
  hrl: "erlang",
  r: "r",
  clj: "clojure",
  cljs: "clojure",
  cljc: "clojure",
  edn: "clojure",
  proto: "protobuf",
  cmake: "cmake",
  v: "verilog",
  sv: "verilog",
  svh: "verilog",
  vh: "verilog",
  vhd: "vhdl",
  vhdl: "vhdl",
  graphql: "graphql",
  gql: "graphql",
  graphqls: "graphql",
  sol: "solidity",
  dot: "dot",
  gv: "dot",
  gd: "gdscript",
  gdscript: "gdscript",
  tscn: "godot",
  tres: "godot",
  godot: "godot",
  liquid: "liquid",
  wat: "wast",
  wast: "wast",
  dockerfile: "dockerfile",
  s: "gas",
  asm: "gas",
  ml: "ocaml",
  mli: "ocaml",
  mll: "ocaml",
  mly: "ocaml",
  fs: "fsharp",
  fsi: "fsharp",
  fsx: "fsharp",
  sml: "sml",
  sig: "sml",
  coffee: "coffeescript",
  litcoffee: "coffeescript",
  cr: "crystal",
  d: "d",
  elm: "elm",
  f: "fortran",
  for: "fortran",
  f77: "fortran",
  f90: "fortran",
  f95: "fortran",
  f03: "fortran",
  f08: "fortran",
  groovy: "groovy",
  gradle: "groovy",
  jl: "julia",
  pas: "pascal",
  dpr: "pascal",
  lpr: "pascal",
  pp: "puppet",
  pug: "pug",
  jade: "pug",
  scm: "scheme",
  ss: "scheme",
  sls: "scheme",
  tcl: "tcl",
  vb: "vb",
  vbs: "vb",
  tex: "latex",
  sty: "latex",
  cls: "latex",
  nginx: "nginx",
  webidl: "webidl",
  properties: "properties",
  astro: "astro",
  hcl: "hcl",
  tf: "hcl",
  tfvars: "hcl",
  zig: "zig",
  zon: "zig",
  jsonc: "json",
  json5: "json",
  yaml: "yaml",
  yml: "yaml",
  toml: "toml",
  sh: "shell",
  bash: "shell",
  zsh: "shell",
  diff: "diff",
  patch: "diff",
  ps1: "powershell",
  psm1: "powershell",
  psd1: "powershell",
};

/**
 * Synchronous extension → mode lookup (drives AUTO). Built-ins win over plugin
 * extensions; returns null for anything unknown, which the caller turns into
 * PlainText — never throws.
 */
export function modeIdForExt(ext: string): ModeChoice | null {
  const e = (ext ?? "").trim().toLowerCase();
  return EXT_MODES[e] ?? null;
}

/** Whole-file names that do not carry a useful extension. */
const FILE_MODES: Record<string, ModeChoice> = {
  dockerfile: "dockerfile",
  containerfile: "dockerfile",
  "cmakelists.txt": "cmake",
  "nginx.conf": "nginx",
  jenkinsfile: "groovy",
  gemfile: "ruby",
  rakefile: "ruby",
  vagrantfile: "ruby",
};

/** Extension-less files (`Dockerfile`, `CMakeLists.txt`, …) → mode id. */
export function modeIdForFile(path: string): ModeChoice | null {
  const base = (path.split(/[\\/]/).pop() ?? "").toLowerCase();
  if (FILE_MODES[base]) return FILE_MODES[base];
  if (base.startsWith("dockerfile.")) return "dockerfile";
  return null;
}

function extOfPath(path: string): string {
  const base = path.split(/[\\/]/).pop() ?? "";
  const dot = base.lastIndexOf(".");
  if (dot <= 0) return "";
  return base.slice(dot + 1).toLowerCase();
}

/** `id -> "plugin:<id>"` — the mode id space never collides with built-ins. */
export function pluginModeId(id: string): ModeId {
  return `plugin:${id}`;
}

interface PluginMode {
  label: string;
  exts: string[];
  support: LanguageSupport;
}

const pluginModes = new Map<string, PluginMode>();
const pluginExts = new Map<string, string>(); // ext (lowercased) -> mode id

/**
 * Compile manifests from the shell into stream languages, replacing any
 * previous set (a reload is a full swap). Rules that fail to compile as JS
 * regexes are dropped with a warning (Rust validated shape, not JS dialect).
 * Returns user-facing warning strings.
 */
export function registerPluginLanguages(plugins: SyntaxPlugin[]): string[] {
  pluginModes.clear();
  pluginExts.clear();
  const warnings: string[] = [];
  for (const p of plugins) {
    const support = buildPluginLanguage(p, warnings);
    if (!support) continue;
    const id = pluginModeId(p.id);
    pluginModes.set(id, { label: p.name, exts: p.extensions, support });
    for (const ext of p.extensions) {
      const key = ext.toLowerCase();
      if (!pluginExts.has(key)) pluginExts.set(key, id); // first plugin wins
    }
  }
  return warnings;
}

/**
 * AUTO resolution over built-ins AND plugins: built-ins win so a plugin can
 * never hijack a language the app ships.
 */
export function autoModeId(ext: string): ModeId | null {
  return modeIdForExt(ext) ?? (pluginExts.get((ext ?? "").trim().toLowerCase()) as ModeId | undefined) ?? null;
}

/**
 * AUTO for a full path: whole-file names win, then the extension (built-ins
 * before plugin extensions).
 */
export function autoModeIdForPath(path: string): ModeId | null {
  return modeIdForFile(path) ?? autoModeId(extOfPath(path));
}

/** Status-bar dropdown entries: built-ins first, then plugin modes. */
export function syntaxModes(): Array<{ id: ModeId; label: string }> {
  const builtin = AVAILABLE_MODES.map((id) => ({ id: id as ModeId, label: MODE_LABELS[id] }));
  const extra = [...pluginModes.entries()].map(([id, m]) => ({ id: id as ModeId, label: m.label }));
  return [...builtin, ...extra];
}

function buildPluginLanguage(
  p: SyntaxPlugin,
  warnings: string[],
): LanguageSupport | null {
  // Sticky (`y`) regexes anchored at the stream position — `exec` on the full
  // line, advancing the stream manually when a rule matches.
  const rules: Array<{ re: RegExp; tag: string }> = [];
  p.rules.forEach((rule, i) => {
    try {
      rules.push({ re: new RegExp(rule.regex, "y"), tag: rule.tag });
    } catch {
      warnings.push(`语法插件「${p.name}」：第 ${i + 1} 条规则的正则无法编译，已跳过`);
    }
  });
  const keywords = new Set(p.keywords);
  const comment = p.lineComment;
  const token = (stream: StringStream): string | null => {
    if (stream.eatSpace()) return null;
    if (comment && stream.string.slice(stream.pos).startsWith(comment)) {
      stream.skipToEnd();
      return "comment";
    }
    for (const rule of rules) {
      rule.re.lastIndex = stream.pos;
      const m = rule.re.exec(stream.string);
      if (m && m[0].length > 0) {
        stream.pos += m[0].length;
        return rule.tag;
      }
    }
    const word = /^\w+/.exec(stream.string.slice(stream.pos));
    if (word) {
      stream.pos += word[0].length;
      return keywords.has(word[0]) ? "keyword" : null;
    }
    stream.next();
    return null;
  };
  return new LanguageSupport(StreamLanguage.define({ token }));
}

const loaded = new Map<ModeChoice, LanguageSupport | null>();
const inFlight = new Map<ModeChoice, Promise<LanguageSupport | null>>();

/**
 * Load (once) the language for a mode. `auto`/`plain`/null → null = PlainText.
 * Plugin modes resolve synchronously from the registry (they were compiled at
 * load time); built-in grammars are imported on demand and cached.
 */
export async function resolveLanguage(mode: ModeId | null): Promise<LanguageSupport | null> {
  if (!mode || mode === "auto" || mode === "plain") return null;
  const plugin = pluginModes.get(mode);
  if (plugin) return plugin.support;
  const builtin = mode as ModeChoice;
  const done = loaded.get(builtin);
  if (done !== undefined) return done;
  let p = inFlight.get(builtin);
  if (!p) {
    p = loadLanguage(builtin).then((lang) => {
      loaded.set(builtin, lang);
      inFlight.delete(builtin);
      return lang;
    });
    inFlight.set(builtin, p);
  }
  return p;
}

async function loadLanguage(mode: ModeChoice): Promise<LanguageSupport | null> {
  switch (mode) {
    case "log":
      return new LanguageSupport(logLanguage, [syntaxHighlighting(streamHighlight)]);
    case "ini":
      return new LanguageSupport(iniLanguage, [syntaxHighlighting(streamHighlight)]);
    case "json": {
      const { json } = await import("@codemirror/lang-json");
      return json();
    }
    case "c": {
      const { cpp } = await import("@codemirror/lang-cpp");
      return cpp();
    }
    case "javascript": {
      const { javascript } = await import("@codemirror/lang-javascript");
      return javascript();
    }
    case "jsx": {
      const { javascript } = await import("@codemirror/lang-javascript");
      return javascript({ jsx: true });
    }
    case "typescript": {
      const { javascript } = await import("@codemirror/lang-javascript");
      return javascript({ typescript: true });
    }
    case "tsx": {
      const { javascript } = await import("@codemirror/lang-javascript");
      return javascript({ typescript: true, jsx: true });
    }
    case "python": {
      const { python } = await import("@codemirror/lang-python");
      return python();
    }
    case "php": {
      const [{ php }, { htmlLanguage }] = await Promise.all([
        import("@codemirror/lang-php"),
        import("@codemirror/lang-html"),
      ]);
      return php({ baseLanguage: htmlLanguage });
    }
    case "html": {
      const { html } = await import("@codemirror/lang-html");
      return html();
    }
    case "vue": {
      const { vue } = await import("@codemirror/lang-vue");
      return vue();
    }
    case "svelte": {
      const { svelte } = await import("@replit/codemirror-lang-svelte");
      return svelte();
    }
    case "css": {
      const { css } = await import("@codemirror/lang-css");
      return css();
    }
    case "scss": {
      const { sass } = await import("@codemirror/lang-sass");
      return sass();
    }
    case "sass": {
      const { sass } = await import("@codemirror/lang-sass");
      return sass({ indented: true });
    }
    case "less": {
      const { less } = await import("@codemirror/lang-less");
      return less();
    }
    case "xml": {
      const { xml } = await import("@codemirror/lang-xml");
      return xml();
    }
    case "markdown": {
      const { markdown } = await import("@codemirror/lang-markdown");
      return markdown();
    }
    case "sql": {
      const { sql } = await import("@codemirror/lang-sql");
      return sql();
    }
    case "rust": {
      const { rust } = await import("@codemirror/lang-rust");
      return rust();
    }
    case "java": {
      const { java } = await import("@codemirror/lang-java");
      return java();
    }
    case "go":
      return streamMode(import("@codemirror/legacy-modes/mode/go"), "go");
    case "csharp":
    case "kotlin":
    case "scala":
    case "dart":
    case "objectivec":
      return clikeMode(mode);
    case "swift":
      return streamMode(import("@codemirror/legacy-modes/mode/swift"), "swift");
    case "ruby":
      return streamMode(import("@codemirror/legacy-modes/mode/ruby"), "ruby");
    case "lua":
      return streamMode(import("@codemirror/legacy-modes/mode/lua"), "lua");
    case "perl":
      return streamMode(import("@codemirror/legacy-modes/mode/perl"), "perl");
    case "haskell":
      return streamMode(import("@codemirror/legacy-modes/mode/haskell"), "haskell");
    case "erlang":
      return streamMode(import("@codemirror/legacy-modes/mode/erlang"), "erlang");
    case "r":
      return streamMode(import("@codemirror/legacy-modes/mode/r"), "r");
    case "clojure":
      return streamMode(import("@codemirror/legacy-modes/mode/clojure"), "clojure");
    case "protobuf":
      return streamMode(import("@codemirror/legacy-modes/mode/protobuf"), "protobuf");
    case "cmake":
      return streamMode(import("@codemirror/legacy-modes/mode/cmake"), "cmake");
    case "verilog":
      return streamMode(import("@codemirror/legacy-modes/mode/verilog"), "verilog");
    case "vhdl":
      return streamMode(import("@codemirror/legacy-modes/mode/vhdl"), "vhdl");
    case "graphql": {
      const { graphqlLanguageSupport } = await import("cm6-graphql");
      return graphqlLanguageSupport();
    }
    case "solidity": {
      const { solidity } = await import("@replit/codemirror-lang-solidity");
      return solidity;
    }
    case "dot": {
      const { dot } = await import("@viz-js/lang-dot");
      return dot();
    }
    case "gdscript": {
      const { simpleMode } = await import("@codemirror/legacy-modes/mode/simple-mode");
      const keywords =
        "if|elif|else|for|while|match|break|continue|pass|return|class|class_name|extends|is|in|as|self|super|signal|func|static|const|enum|var|breakpoint|preload|await|yield|assert|true|false|null|and|or|not|void|PI|TAU|INF|NAN";
      const types =
        "int|float|bool|String|StringName|NodePath|Vector2|Vector2i|Vector3|Vector3i|Vector4|Vector4i|Color|Dictionary|Array|PackedByteArray|PackedInt32Array|PackedInt64Array|PackedFloat32Array|PackedFloat64Array|PackedStringArray|PackedVector2Array|PackedVector3Array|PackedColorArray|Rect2|Rect2i|Transform2D|Transform3D|Basis|Quaternion|AABB|Plane|Callable|Signal|Object|RefCounted|Resource|Node|Node2D|Node3D|Control|CanvasItem|CanvasLayer|SceneTree|Window|Viewport|Camera2D|Camera3D|Sprite2D|Sprite3D|AnimatedSprite2D|CharacterBody2D|CharacterBody3D|RigidBody2D|RigidBody3D|StaticBody2D|StaticBody3D|Area2D|Area3D|CollisionShape2D|CollisionShape3D|RectangleShape2D|CircleShape2D|CapsuleShape2D|Label|Button|LineEdit|TextEdit|RichTextLabel|Texture2D|TextureRect|PackedScene|Timer|Tween|AudioStreamPlayer|AudioStreamPlayer2D|AudioStreamPlayer3D|AnimationPlayer|AnimationTree|TileMap|TileMapLayer|Input|InputEvent|OS|Engine|ProjectSettings|FileAccess|DirAccess|JSON|RandomNumberGenerator|HTTPRequest|HTTPClient|Thread|Mutex|Semaphore";
      const builtins =
        "print|printerr|printraw|print_debug|print_rich|printt|prints|printv|push_error|push_warning|str|int|float|bool|len|range|load|preload|instantiate|connect|emit_signal|call|call_deferred|get_node|get_parent|queue_free|randi|randf|randi_range|randf_range|rand_from_seed|clamp|clampi|clampf|lerp|lerpf|abs|absi|absf|min|mini|minf|max|maxi|maxf|round|roundi|floor|floori|ceil|ceili|sqrt|pow|sin|cos|tan|asin|acos|atan|atan2|deg_to_rad|rad_to_deg|wrapi|wrapf|snapped|snappedi|snappedf|sign|signi|signf|is_instance_valid|weakref|type_exists|is_instance_of|hash|instance_from_id|breakpoint";
      return new LanguageSupport(
        StreamLanguage.define(
          simpleMode({
            start: [
              { regex: /#[^\n]*/, token: "comment" },
              { regex: /[&^r]?"""/, token: "string", push: "tripleDouble" },
              { regex: /[&^r]?'''/, token: "string", push: "tripleSingle" },
              { regex: /[&^r]?"/, token: "string", push: "double" },
              { regex: /[&^r]?'/, token: "string", push: "single" },
              { regex: /@[A-Za-z_]\w*/, token: "meta" },
              { regex: /\$[A-Za-z0-9_\/%.]+/, token: "variableName" },
              { regex: /%[A-Za-z_]\w*/, token: "variableName" },
              { regex: /0[xX][0-9a-fA-F_]+/, token: "number" },
              { regex: /0[bB][01_]+/, token: "number" },
              { regex: /\d[\d_]*(?:\.[\d_]*)?(?:[eE][+-]?\d+)?/, token: "number" },
              { regex: new RegExp(`\\b(?:${keywords})\\b`), token: "keyword" },
              { regex: new RegExp(`\\b(?:${types})\\b`), token: "typeName" },
              { regex: new RegExp(`\\b(?:${builtins})\\b`), token: "variableName" },
              { regex: /\b[A-Za-z_]\w*(?=\s*\()/, token: "variableName" },
              { regex: /\b[A-Z][A-Za-z0-9_]*\b/, token: "typeName" },
              { regex: /[{}()[\]]/, token: "punctuation" },
              { regex: /[+\-*/%<>=!&|^~?:]/, token: "operator" },
              { regex: /\b[A-Za-z_]\w*/, token: null },
            ],
            double: [
              { regex: /[^"\\]+/, token: "string" },
              { regex: /\\./, token: "string" },
              { regex: /"/, token: "string", pop: true },
            ],
            single: [
              { regex: /[^'\\]+/, token: "string" },
              { regex: /\\./, token: "string" },
              { regex: /'/, token: "string", pop: true },
            ],
            tripleDouble: [
              { regex: /"""/, token: "string", pop: true },
              { regex: /[^"]+/, token: "string" },
              { regex: /"+/, token: "string" },
            ],
            tripleSingle: [
              { regex: /'''/, token: "string", pop: true },
              { regex: /[^']+/, token: "string" },
              { regex: /'+/, token: "string" },
            ],
          }),
        ),
      );
    }
    case "liquid": {
      const { liquid } = await import("@codemirror/lang-liquid");
      return liquid();
    }
    case "angular": {
      const { angular } = await import("@codemirror/lang-angular");
      return angular();
    }
    case "wast": {
      const { wast } = await import("@codemirror/lang-wast");
      return wast();
    }
    case "dockerfile":
      return streamMode(import("@codemirror/legacy-modes/mode/dockerfile"), "dockerFile");
    case "gas":
      return streamMode(import("@codemirror/legacy-modes/mode/gas"), "gas");
    case "ocaml":
      return streamMode(import("@codemirror/legacy-modes/mode/mllike"), "oCaml");
    case "fsharp":
      return streamMode(import("@codemirror/legacy-modes/mode/mllike"), "fSharp");
    case "sml":
      return streamMode(import("@codemirror/legacy-modes/mode/mllike"), "sml");
    case "coffeescript":
      return streamMode(import("@codemirror/legacy-modes/mode/coffeescript"), "coffeeScript");
    case "crystal":
      return streamMode(import("@codemirror/legacy-modes/mode/crystal"), "crystal");
    case "d":
      return streamMode(import("@codemirror/legacy-modes/mode/d"), "d");
    case "elm":
      return streamMode(import("@codemirror/legacy-modes/mode/elm"), "elm");
    case "fortran":
      return streamMode(import("@codemirror/legacy-modes/mode/fortran"), "fortran");
    case "groovy":
      return streamMode(import("@codemirror/legacy-modes/mode/groovy"), "groovy");
    case "julia":
      return streamMode(import("@codemirror/legacy-modes/mode/julia"), "julia");
    case "pascal":
      return streamMode(import("@codemirror/legacy-modes/mode/pascal"), "pascal");
    case "puppet":
      return streamMode(import("@codemirror/legacy-modes/mode/puppet"), "puppet");
    case "pug":
      return streamMode(import("@codemirror/legacy-modes/mode/pug"), "pug");
    case "scheme":
      return streamMode(import("@codemirror/legacy-modes/mode/scheme"), "scheme");
    case "tcl":
      return streamMode(import("@codemirror/legacy-modes/mode/tcl"), "tcl");
    case "vb":
      return streamMode(import("@codemirror/legacy-modes/mode/vb"), "vb");
    case "latex":
      return streamMode(import("@codemirror/legacy-modes/mode/stex"), "stex");
    case "nginx":
      return streamMode(import("@codemirror/legacy-modes/mode/nginx"), "nginx");
    case "webidl":
      return streamMode(import("@codemirror/legacy-modes/mode/webidl"), "webIDL");
    case "properties":
      return streamMode(import("@codemirror/legacy-modes/mode/properties"), "properties");
    case "godot": {
      const { simpleMode } = await import("@codemirror/legacy-modes/mode/simple-mode");
      const types =
        "Vector2|Vector2i|Vector3|Vector3i|Vector4|Vector4i|Color|Rect2|Rect2i|Transform2D|Transform3D|Basis|Quaternion|AABB|Plane|NodePath|PackedVector2Array|PackedVector3Array|PackedColorArray|PackedByteArray|PackedInt32Array|PackedInt64Array|PackedFloat32Array|PackedFloat64Array|PackedStringArray";
      return new LanguageSupport(
        StreamLanguage.define(
          simpleMode({
            start: [
              { regex: /[;#][^\n]*/, token: "comment" },
              { regex: /\[[A-Za-z_]\w*/, token: "className" },
              { regex: /[A-Za-z_][\w/]*(?=\s*=)/, token: "propertyName" },
              { regex: /[&^]?"/, token: "string", push: "double" },
              { regex: /\b(?:true|false|null)\b/, token: "atom" },
              { regex: new RegExp(`\\b(?:${types})\\b`), token: "typeName" },
              { regex: /\b(?:ExtResource|SubResource)\b/, token: "variableName" },
              { regex: /-?\d[\d_]*(?:\.[\d_]*)?(?:[eE][+-]?\d+)?/, token: "number" },
              { regex: /[{}()[\]]/, token: "punctuation" },
              { regex: /[=,:]/, token: "operator" },
              { regex: /\b[A-Za-z_]\w*/, token: null },
            ],
            double: [
              { regex: /[^"\\]+/, token: "string" },
              { regex: /\\./, token: "string" },
              { regex: /"/, token: "string", pop: true },
            ],
          }),
        ),
      );
    }
    case "astro":
      return astroMode();
    case "hcl":
      return hclMode();
    case "zig":
      return zigMode();
    case "yaml":
      return streamMode(import("@codemirror/legacy-modes/mode/yaml"), "yaml");
    case "toml":
      return streamMode(import("@codemirror/legacy-modes/mode/toml"), "toml");
    case "shell":
      return streamMode(import("@codemirror/legacy-modes/mode/shell"), "shell");
    case "diff":
      return streamMode(import("@codemirror/legacy-modes/mode/diff"), "diff");
    case "powershell":
      return streamMode(import("@codemirror/legacy-modes/mode/powershell"), "powerShell");
    default:
      return null;
  }
}

/** Wrap a CodeMirror 5 style stream parser (`@codemirror/legacy-modes`). */
async function streamMode(
  mod: Promise<Record<string, unknown>>,
  exportName: string,
): Promise<LanguageSupport> {
  const spec = (await mod)[exportName] as StreamParser<unknown>;
  return new LanguageSupport(StreamLanguage.define(spec));
}
/** Legacy `clike` packs several C-family dialects behind one module. */
const CLIKE_EXPORTS: Partial<Record<ModeChoice, string>> = {
  csharp: "csharp",
  kotlin: "kotlin",
  scala: "scala",
  dart: "dart",
  objectivec: "objectiveC",
};

async function clikeMode(mode: ModeChoice): Promise<LanguageSupport> {
  const mod = await import("@codemirror/legacy-modes/mode/clike");
  const spec = (mod as unknown as Record<string, StreamParser<unknown>>)[
    CLIKE_EXPORTS[mode]!
  ];
  return new LanguageSupport(StreamLanguage.define(spec));
}
/** Astro: `---` frontmatter (TS/JS), HTML template, `{}` expressions. */
async function astroMode(): Promise<LanguageSupport> {
  const { simpleMode } = await import("@codemirror/legacy-modes/mode/simple-mode");
  const jsKeywords =
    "abstract|as|async|await|break|case|catch|class|const|continue|debugger|declare|default|delete|do|else|enum|export|extends|false|finally|for|from|function|get|if|implements|import|in|instanceof|interface|let|new|null|of|private|protected|public|readonly|return|satisfies|set|static|super|switch|this|throw|true|try|type|typeof|undefined|var|void|while|with|yield";
  const keywordRe = new RegExp(`\\b(?:${jsKeywords})\\b`);
  const jsRules = (popBrace: boolean) => [
    { regex: /\/\/[^\n]*/, token: "comment" },
    { regex: /\/\*/, token: "comment", push: "blockComment" },
    { regex: /`(?:[^`\\]|\\.)*`/, token: "string" },
    { regex: /"(?:[^"\\]|\\.)*"/, token: "string" },
    { regex: /'(?:[^'\\]|\\.)*'/, token: "string" },
    { regex: keywordRe, token: "keyword" },
    { regex: /\b\d[\d_]*(?:\.\d+)?(?:[eE][+-]?\d+)?\b/, token: "number" },
    { regex: /{/, token: "punctuation", push: "expr" },
    ...(popBrace
      ? [{ regex: /}/, token: "punctuation", pop: true }]
      : [{ regex: /}/, token: "punctuation" }]),
    { regex: /[{}[\]()]/, token: "punctuation" },
    { regex: /\b[A-Za-z_$][\w$]*(?=\s*\()/, token: "variableName" },
    { regex: /\b[A-Za-z_$][\w$]*/, token: null },
    { regex: /[+\-*/%<>=!&|^~?:;,.]+/, token: "operator" },
  ];
  return new LanguageSupport(
    StreamLanguage.define(
      simpleMode({
        start: [
          { regex: /^---\s*$/, sol: true, token: "punctuation", push: "frontmatter" },
          { regex: /<!--/, token: "comment", push: "htmlComment" },
          { regex: /<\/?[A-Za-z][\w.:-]*/, token: "tag", push: "tag" },
          { regex: /{/, token: "punctuation", push: "expr" },
          { regex: /[^<{]+/, token: null },
          { regex: /</, token: "punctuation" },
        ],
        frontmatter: [
          { regex: /^---\s*$/, sol: true, token: "punctuation", pop: true },
          ...jsRules(false),
        ],
        expr: jsRules(true),
        tag: [
          { regex: /\/?>/, token: "punctuation", pop: true },
          { regex: /[A-Za-z_:][\w:.-]*(?=\s*=)/, token: "attribute" },
          { regex: /[A-Za-z_:][\w:.-]*/, token: "attribute" },
          { regex: /=/, token: "operator" },
          { regex: /"/, token: "string", push: "attrDouble" },
          { regex: /'/, token: "string", push: "attrSingle" },
          { regex: /{/, token: "punctuation", push: "expr" },
          { regex: /\s+/, token: null },
          { regex: /[^\s>"'{]+/, token: "attribute" },
        ],
        attrDouble: [
          { regex: /[^"]+/, token: "string" },
          { regex: /"/, token: "string", pop: true },
        ],
        attrSingle: [
          { regex: /[^']+/, token: "string" },
          { regex: /'/, token: "string", pop: true },
        ],
        htmlComment: [
          { regex: /-->/, token: "comment", pop: true },
          { regex: /[^-]+/, token: "comment" },
          { regex: /-/, token: "comment" },
        ],
        blockComment: [
          { regex: /\*\//, token: "comment", pop: true },
          { regex: /[^*]+/, token: "comment" },
          { regex: /\*/, token: "comment" },
        ],
      }),
    ),
  );
}

/** HCL / Terraform: blocks, labels, `key = value`, strings, `${}` interpolation. */
async function hclMode(): Promise<LanguageSupport> {
  const { simpleMode } = await import("@codemirror/legacy-modes/mode/simple-mode");
  const keywords =
    "resource|data|variable|output|module|provider|terraform|locals|dynamic|for_each|count|depends_on|lifecycle|provisioner|connection|backend|required_providers|required_version|source|version|content|when|true|false|null|if|for|in|else|endif|endfor";
  const types = "string|number|bool|list|map|set|tuple|object|any|optional";
  const builtins =
    "var|local|module|data|each|count|path|terraform|self|jsonencode|jsondecode|file|templatefile|lookup|merge|concat|length|toset|tolist|tomap|tostring|tonumber|tobool|try|can|coalesce|format|formatlist|join|split|replace|regex|regexall|upper|lower|trim|trimspace|trimprefix|trimsuffix|sensitive|nonsensitive|timestamp|uuid|base64encode|base64decode|cidrsubnet|cidrhost|flatten|distinct|keys|values|zipmap|contains|startswith|endswith";
  return new LanguageSupport(
    StreamLanguage.define(
      simpleMode({
        start: [
          { regex: /#[^\n]*/, token: "comment" },
          { regex: /\/\/[^\n]*/, token: "comment" },
          { regex: /\/\*/, token: "comment", push: "blockComment" },
          { regex: /"/, token: "string", push: "double" },
          { regex: /<<-?[A-Za-z_]\w*/, token: "string", push: "heredoc" },
          { regex: new RegExp(`\\b(?:${keywords})\\b`), token: "keyword" },
          { regex: new RegExp(`\\b(?:${types})\\b`), token: "typeName" },
          { regex: new RegExp(`\\b(?:${builtins})\\b`), token: "variableName" },
          { regex: /\b[A-Za-z_][\w-]*(?=\s*=)/, token: "propertyName" },
          { regex: /\b[A-Za-z_][\w-]*/, token: null },
          { regex: /\b\d[\d_]*(?:\.\d+)?(?:[eE][+-]?\d+)?/, token: "number" },
          { regex: /[{}[\]()]/, token: "punctuation" },
          { regex: /[=,:.]/, token: "operator" },
          { regex: /[+\-*/%<>=!&|?]+/, token: "operator" },
        ],
        double: [
          { regex: /[^"\\$]+/, token: "string" },
          { regex: /\\./, token: "string" },
          { regex: /\$\{/, token: "string", push: "interp" },
          { regex: /"/, token: "string", pop: true },
        ],
        interp: [
          { regex: /}/, token: "punctuation", pop: true },
          { regex: /{/, token: "punctuation", push: "interp" },
          { regex: /"(?:[^"\\]|\\.)*"/, token: "string" },
          { regex: new RegExp(`\\b(?:${builtins})\\b`), token: "variableName" },
          {
            regex: /\b(?:var|local|module|data|each|count|path|terraform|self|true|false|null|if|for|in)\b/,
            token: "keyword",
          },
          { regex: /\b[A-Za-z_]\w*/, token: null },
          { regex: /\d[\d_]*(?:\.\d+)?/, token: "number" },
          { regex: /[{}[\]()]/, token: "punctuation" },
          { regex: /[=,:.]/, token: "operator" },
        ],
        heredoc: [
          { regex: /^[A-Za-z_]\w*\s*$/, sol: true, token: "string", pop: true },
          { regex: /[^\n]+/, token: "string" },
        ],
        blockComment: [
          { regex: /\*\//, token: "comment", pop: true },
          { regex: /[^*]+/, token: "comment" },
          { regex: /\*/, token: "comment" },
        ],
      }),
    ),
  );
}

/** Zig: keywords, primitive types, `@builtins`, error sets, multiline strings. */
async function zigMode(): Promise<LanguageSupport> {
  const { simpleMode } = await import("@codemirror/legacy-modes/mode/simple-mode");
  const keywords =
    "const|var|fn|pub|export|extern|inline|noinline|comptime|usingnamespace|struct|enum|union|opaque|error|test|if|else|switch|while|for|break|continue|return|defer|errdefer|try|catch|orelse|unreachable|async|await|suspend|resume|nosuspend|asm|volatile|linksection|threadlocal|anytype|anyframe|align|packed|callconv|noalias|and|or|not|undefined|null|true|false";
  const types =
    "bool|void|noreturn|type|anyerror|comptime_int|comptime_float|i8|i16|i32|i64|i128|u8|u16|u32|u64|u128|isize|usize|f16|f32|f64|f80|f128|c_char|c_short|c_ushort|c_int|c_uint|c_long|c_ulong|c_longlong|c_ulonglong|c_longdouble";
  return new LanguageSupport(
    StreamLanguage.define(
      simpleMode({
        start: [
          { regex: /\/\/[^\n]*/, token: "comment" },
          { regex: /\\\\[^\n]*/, token: "string" },
          { regex: /"(?:[^"\\]|\\.)*"/, token: "string" },
          { regex: /'(?:[^'\\]|\\.)*'/, token: "string" },
          { regex: /@"[^"]*"/, token: "variableName" },
          { regex: /@[A-Za-z_]\w*/, token: "variableName" },
          { regex: /error\{[^}]*\}/, token: "typeName" },
          { regex: new RegExp(`\\b(?:${keywords})\\b`), token: "keyword" },
          { regex: new RegExp(`\\b(?:${types})\\b`), token: "typeName" },
          { regex: /\b0[xX][0-9a-fA-F_]+/, token: "number" },
          { regex: /\b0[oO][0-7_]+/, token: "number" },
          { regex: /\b0[bB][01_]+/, token: "number" },
          { regex: /\b\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d+)?/, token: "number" },
          { regex: /[{}[\]()]/, token: "punctuation" },
          { regex: /[+\-*/%<>=!&|^~?:.]+/, token: "operator" },
          { regex: /\b[A-Za-z_]\w*/, token: null },
        ],
      }),
    ),
  );
}
