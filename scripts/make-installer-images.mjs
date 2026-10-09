// 生成 Windows NSIS 安装向导的品牌素材（侧栏图 164×314、头图 150×57，按 2 倍输出）。
// 用法：pnpm installer-images
// 素材源自 backend/app-icon.svg 的插头线条图标，配色沿用应用的黑白基调（#111111 / #ffffff）。
// NSIS 只认 BMP，本脚本用 @resvg/resvg-js 把 SVG 渲染成像素，再手写 24 位 BMP 文件。
// 输出为 2 倍分辨率：向导使用微软雅黑后对话框变大，再叠加系统 DPI 缩放，
// 1 倍位图会被 NSIS 拉伸放大而发糊；2 倍位图改为缩小显示，边缘保持清晰。
import { Resvg } from "@resvg/resvg-js";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const outDir = join(root, "backend", "installer");
const previewDir = process.argv[2] ?? null;
const SCALE = 2;

const BG = "#111111";
const INK = "#ffffff";
const MUTED = "#8e8e8b";
const LINE = "#2b2b2b";
const FONT_SANS = "Segoe UI, Microsoft YaHei, sans-serif";
const FONT_CJK = "Microsoft YaHei, Segoe UI, sans-serif";

// 与 backend/app-icon.svg 相同的 24×24 线条图标
const ICON_PATHS = `
  <path d="M6.3 20.3a2.4 2.4 0 0 0 3.4 0L12 18l-6-6-2.3 2.3a2.4 2.4 0 0 0 0 3.4Z"/>
  <path d="m2 22 3-3"/>
  <path d="M7.5 13.5 10 11"/>
  <path d="M10.5 16.5 13 14"/>
  <path d="m18 3-4 4h6l-4 4"/>`;

/** 把 24 单位的图标放到 (x, y)，缩放到 size 像素 */
function icon(x, y, size, strokeWidth = 2) {
  const s = size / 24;
  return `<g transform="translate(${x} ${y}) scale(${s})" fill="none" stroke="${INK}"
    stroke-width="${strokeWidth}" stroke-linecap="round" stroke-linejoin="round">${ICON_PATHS}</g>`;
}

/** 欢迎页 / 完成页左侧栏：图标居中，下方品牌名与一句中文说明 */
function sidebarSvg(w, h) {
  const iconSize = 60;
  const ix = (w - iconSize) / 2;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">
  <rect width="${w}" height="${h}" fill="${BG}"/>
  ${icon(ix, 78, iconSize, 1.9)}
  <text x="${w / 2}" y="186" text-anchor="middle" font-family="${FONT_SANS}" font-size="19" font-weight="600" fill="${INK}">port-helper</text>
  <text x="${w / 2}" y="210" text-anchor="middle" font-family="${FONT_CJK}" font-size="11" fill="${MUTED}">端口占用查询与进程管理</text>
  <rect x="${w / 2 - 12}" y="${h - 40}" width="24" height="1" fill="${LINE}"/>
</svg>`;
}

/** 安装位置 / 进度页顶部头图：图标在左，品牌名在右 */
function headerSvg(w, h) {
  const iconSize = 26;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">
  <rect width="${w}" height="${h}" fill="${BG}"/>
  ${icon(18, (h - iconSize) / 2, iconSize, 2)}
  <text x="54" y="${h / 2 + 6}" font-family="${FONT_SANS}" font-size="16" font-weight="600" fill="${INK}">port-helper</text>
</svg>`;
}

/** RGBA 像素 → 24 位 BMP（行自下而上、每行 4 字节对齐、BGR 顺序），透明部分压在 BG 上 */
function toBmp(width, height, rgba) {
  const bgR = 0x11, bgG = 0x11, bgB = 0x11;
  const rowSize = Math.ceil((width * 3) / 4) * 4;
  const pixelBytes = rowSize * height;
  const headerSize = 14 + 40;
  const buf = Buffer.alloc(headerSize + pixelBytes);
  // BITMAPFILEHEADER
  buf.write("BM", 0, "ascii");
  buf.writeUInt32LE(headerSize + pixelBytes, 2);
  buf.writeUInt32LE(0, 6);
  buf.writeUInt32LE(headerSize, 10);
  // BITMAPINFOHEADER
  buf.writeUInt32LE(40, 14);
  buf.writeInt32LE(width, 18);
  buf.writeInt32LE(height, 22);
  buf.writeUInt16LE(1, 26);
  buf.writeUInt16LE(24, 28);
  buf.writeUInt32LE(0, 30);
  buf.writeUInt32LE(pixelBytes, 34);
  buf.writeInt32LE(2835, 38);
  buf.writeInt32LE(2835, 42);
  buf.writeUInt32LE(0, 46);
  buf.writeUInt32LE(0, 50);
  for (let y = 0; y < height; y++) {
    const srcRow = height - 1 - y;
    let off = headerSize + y * rowSize;
    for (let x = 0; x < width; x++) {
      const i = (srcRow * width + x) * 4;
      const a = rgba[i + 3] / 255;
      buf[off++] = Math.round(rgba[i + 2] * a + bgB * (1 - a));
      buf[off++] = Math.round(rgba[i + 1] * a + bgG * (1 - a));
      buf[off++] = Math.round(rgba[i] * a + bgR * (1 - a));
    }
  }
  return buf;
}

/** 按设计尺寸（1 倍）画 SVG，渲染时整体放大 SCALE 倍 */
function render(name, svg, baseWidth, baseHeight) {
  const width = baseWidth * SCALE;
  const height = baseHeight * SCALE;
  const resvg = new Resvg(svg, {
    fitTo: { mode: "zoom", value: SCALE },
    font: { loadSystemFonts: true, defaultFontFamily: "Segoe UI" },
  });
  const img = resvg.render();
  if (img.width !== width || img.height !== height) {
    throw new Error(`${name} 尺寸不符：期望 ${width}×${height}，实际 ${img.width}×${img.height}`);
  }
  const bmpPath = join(outDir, `${name}.bmp`);
  writeFileSync(bmpPath, toBmp(width, height, img.pixels));
  console.log(`已生成 ${bmpPath} (${width}×${height})`);
  if (previewDir) {
    mkdirSync(previewDir, { recursive: true });
    writeFileSync(join(previewDir, `${name}.png`), img.asPng());
  }
}

mkdirSync(outDir, { recursive: true });
render("sidebar", sidebarSvg(164, 314), 164, 314);
render("header", headerSvg(150, 57), 150, 57);
