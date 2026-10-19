// 生成应用内更新清单 latest.json：在发布目录里找出各平台更新包的 .sig 签名，拼出下载地址后写入同一目录
// 用法：node scripts/make-update-manifest.mjs <发布目录> <版本号> <owner/repo> [更新说明文件]
// 更新端点固定为 releases/latest/download/latest.json，见 backend/tauri.conf.json 的 plugins.updater
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";

const [dir, version, repo, notesFile] = process.argv.slice(2);
if (!dir || !/^\d+\.\d+\.\d+$/.test(version ?? "") || !/^[\w.-]+\/[\w.-]+$/.test(repo ?? "")) {
  console.error("用法：node scripts/make-update-manifest.mjs <发布目录> <版本号> <owner/repo> [更新说明文件]");
  process.exit(1);
}

/** 由更新包文件名判断平台键（OS-ARCH）；安装包 .msi / .deb / .rpm 的签名不进清单 */
function platformOf(name) {
  if (name.endsWith("-setup.exe")) return "windows-x86_64";
  if (name.endsWith(".AppImage")) return "linux-x86_64";
  if (name.endsWith(".app.tar.gz")) {
    // macOS 两个架构的更新包在 CI 中已按架构改名，如 port-helper_0.1.5_aarch64.app.tar.gz
    if (name.includes("_aarch64.")) return "darwin-aarch64";
    if (name.includes("_x86_64.")) return "darwin-x86_64";
  }
  return null;
}

const files = readdirSync(dir, { recursive: true })
  .map((rel) => join(dir, rel))
  .filter((f) => statSync(f).isFile());

const platforms = {};
for (const sig of files.filter((f) => f.endsWith(".sig"))) {
  const bundle = sig.slice(0, -".sig".length);
  const name = basename(bundle);
  const platform = platformOf(name);
  if (!platform || !existsSync(bundle)) continue;
  if (platforms[platform]) {
    console.error(`平台 ${platform} 有多个更新包：${platforms[platform].file} 与 ${name}`);
    process.exit(1);
  }
  platforms[platform] = {
    file: name,
    signature: readFileSync(sig, "utf8").trim(),
    url: `https://github.com/${repo}/releases/download/v${version}/${encodeURIComponent(name)}`,
  };
}

if (Object.keys(platforms).length === 0) {
  console.log("没有找到更新包签名（未配置 TAURI_SIGNING_PRIVATE_KEY？），不生成 latest.json");
  process.exit(0);
}

const notes = notesFile && existsSync(notesFile) ? readFileSync(notesFile, "utf8").trim() : "";
const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: Object.fromEntries(Object.entries(platforms).map(([k, { signature, url }]) => [k, { signature, url }])),
};
writeFileSync(join(dir, "latest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
for (const [k, { file }] of Object.entries(platforms)) console.log(`${k} → ${file}`);
