import fs from "node:fs";
import path from "node:path";
import { JSDOM } from "jsdom";
import { lint } from "markdownlint/promise";

const rootDir = path.resolve(process.argv[2] || ".");
const configPath = path.join(path.dirname(new URL(import.meta.url).pathname), ".markdownlint.json");
const lintConfig = JSON.parse(fs.readFileSync(configPath, "utf8"));

const dom = new JSDOM("<!doctype html><html><body></body></html>");
globalThis.window = dom.window;
globalThis.document = dom.window.document;
globalThis.Element = dom.window.Element;
globalThis.HTMLElement = dom.window.HTMLElement;
globalThis.SVGElement = dom.window.SVGElement;
const { default: mermaid } = await import("mermaid");
mermaid.initialize({ startOnLoad: false });

function collectMarkdown(dir, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === "node_modules" || entry.name === "target" || entry.name === ".git") continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      collectMarkdown(full, out);
    } else if (entry.isFile() && entry.name.endsWith(".md")) {
      out.push(full);
    }
  }
  return out;
}

const activeFiles = [];
for (const name of ["README.md", "AGENTS.md", "AGENT.md", "PROJECT.md", "CONTRIBUTING.md", "SECURITY.md", "SYNC_DESIGN.md"]) {
  const p = path.join(rootDir, name);
  if (fs.existsSync(p)) activeFiles.push(p);
}
if (fs.existsSync(path.join(rootDir, "docs"))) {
  for (const p of collectMarkdown(path.join(rootDir, "docs"))) {
    const rel = path.relative(rootDir, p);
    if (!rel.startsWith(`docs${path.sep}archive${path.sep}`)) activeFiles.push(p);
  }
}
if (fs.existsSync(path.join(rootDir, "crates"))) {
  for (const p of collectMarkdown(path.join(rootDir, "crates"))) {
    activeFiles.push(p);
  }
}

let failures = 0;

// 1. Run markdownlint on active docs
const lintResults = await lint({ files: activeFiles, config: lintConfig });
for (const [file, issues] of Object.entries(lintResults)) {
  const rel = path.relative(rootDir, file);
  for (const issue of issues) {
    // Allow MD013/MD033/etc. per config; report remaining structural issues
    console.error(`markdownlint: ${rel}:${issue.lineNumber} ${issue.ruleNames.join("/")} - ${issue.ruleDescription}`);
    failures++;
  }
}

// 2. Run real mermaid.parse() on every Mermaid fence in active docs
for (const file of activeFiles) {
  const rel = path.relative(rootDir, file);
  const lines = fs.readFileSync(file, "utf8").split(/\r?\n/);
  let inMermaid = false;
  let fenceStart = 0;
  let fenceChar = "";
  let fenceLen = 0;
  let buf = [];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const trimmed = line.trim();
    if (!inMermaid) {
      const m = trimmed.match(/^(`{3,}|~{3,})\s*mermaid\b/i);
      if (m) {
        inMermaid = true;
        fenceStart = i + 1;
        fenceChar = m[1][0];
        fenceLen = m[1].length;
        buf = [];
      }
    } else {
      const closeRe = new RegExp(`^${fenceChar}{${fenceLen},}\\s*$`);
      if (closeRe.test(trimmed)) {
        const code = buf.join("\n").trim();
        try {
          await mermaid.parse(code);
        } catch (err) {
          const msg = String(err?.message || err).split("\n")[0];
          console.error(`mermaid.parse: ${rel}:${fenceStart} - ${msg}`);
          failures++;
        }
        inMermaid = false;
      } else {
        buf.push(line);
      }
    }
  }
}

if (failures > 0) {
  console.error(`Node docs validation failed with ${failures} error(s).`);
  process.exit(1);
}
console.log(`Node markdownlint + mermaid.parse PASSED (${activeFiles.length} active files).`);
