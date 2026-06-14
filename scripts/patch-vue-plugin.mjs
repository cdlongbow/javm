// postinstall 补丁：修复 @vitejs/plugin-vue 竞态崩溃
// compiler: null 在 buildStart 初始化之前触发 handleHotUpdate 会导致
// "null is not an object (evaluating 'options.value.compiler.invalidateTypeCache')"
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const targetPath = resolve(
  __dirname,
  "..",
  "node_modules/@vitejs/plugin-vue/dist/index.mjs",
);

let content = readFileSync(targetPath, "utf-8");
const oldLine = "if (options.value.compiler.invalidateTypeCache) options.value.compiler.invalidateTypeCache(ctx.file);";
const newLine = "if (options.value.compiler?.invalidateTypeCache) options.value.compiler.invalidateTypeCache(ctx.file);";

if (content.includes(oldLine)) {
  content = content.replace(oldLine, newLine);
  writeFileSync(targetPath, content, "utf-8");
  console.log("[postinstall] ✅ 已修复 @vitejs/plugin-vue compiler 空指针崩溃");
} else if (content.includes(newLine)) {
  console.log("[postinstall] ✅ @vitejs/plugin-vue 补丁已就绪（无需重复应用）");
} else {
  console.warn("[postinstall] ⚠️ 未找到目标行，补丁可能已过时，请检查 @vitejs/plugin-vue 版本");
}
