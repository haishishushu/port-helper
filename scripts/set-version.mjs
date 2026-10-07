// 把所有清单文件里的应用版本号统一改为指定版本（只替换版本字段，保留原有格式）
// 用法：node scripts/set-version.mjs 0.1.5
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version ?? "")) {
  console.error(`版本号格式错误：${version ?? "(空)"}，应为 x.y.z`);
  process.exit(1);
}

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

// [文件, 匹配版本字段的正则（第 1 组为前缀，第 2 组为后缀）]
const targets = [
  ["backend/tauri.conf.json", /^(\s*"version":\s*")[^"]+(")/m],
  ["package.json", /^(\s*"version":\s*")[^"]+(")/m],
  ["frontend/package.json", /^(\s*"version":\s*")[^"]+(")/m],
  ["backend/Cargo.toml", /^(\[package\][\s\S]*?^version\s*=\s*")[^"]+(")/m],
  ["backend/Cargo.lock", /^(name = "port-helper"\r?\nversion = ")[^"]+(")/m],
];

for (const [file, pattern] of targets) {
  const path = join(root, file);
  const text = readFileSync(path, "utf8");
  if (!pattern.test(text)) {
    console.error(`${file} 中找不到版本字段`);
    process.exit(1);
  }
  writeFileSync(path, text.replace(pattern, `$1${version}$2`));
  console.log(`${file} → ${version}`);
}
