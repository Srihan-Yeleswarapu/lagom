// Loads the real compiler wasm (web/pkg) into Node for tests. The check/run
// JSON returned here is the same JSON the browser worker receives.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const webDir = join(dirname(fileURLToPath(import.meta.url)), "..");
const lagom = await import(pathToFileURL(join(webDir, "pkg", "lagom_web.js")).href);
lagom.initSync({ module: readFileSync(join(webDir, "pkg", "lagom_web_bg.wasm")) });

// Parsed results, matching what runner.worker.js posts back as `result`.
export function compilerCheck(source) {
  return JSON.parse(lagom.check(source));
}

export function compilerRun(source, stdin = "") {
  return JSON.parse(lagom.run(source, stdin));
}
