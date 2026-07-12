// 内容实时预览：卡牌整卡合成 + 遗物/能力/药水悬浮提示样式。
// 卡框素材提取自游戏（经 tutorials.sts2modding.com 的卡框预览工具整理）；
// 换色逻辑逐行移植 RitsuLib MaterialUtils.CreateReplaceHueShaderMaterial 的
// ReplaceHue 着色器（保留原亮度/饱和度，仅替换色相），预览效果与游戏内一致。

// 游戏命名颜色（RichText BBCode 标签）
const STS_COLORS = {
  gold: "#EFC851", red: "#FF5555", green: "#7FFF00", blue: "#87CEEB",
  purple: "#EE82EE", orange: "#FFA518", pink: "#FF78A0", aqua: "#2AEBBE",
};
const STS_FX = new Set(["jitter", "sine", "rainbow", "fade_in", "fly_in", "thinky_dots"]);

const FRAME_TYPES = { Attack: "attack", Skill: "skill", Power: "power", Status: "skill", Curse: "skill" };
const PREVIEW_KINDS = new Set(["cards", "relics", "powers", "potions"]);

const frameImages = {};   // type -> HTMLImageElement（原始卡框）
const recolorCache = {};  // type|color|brightness -> dataURL
const imageCache = {};    // dir|rel -> data URL（项目图片）
let previewShowUpgraded = false;

function loadFrameImage(type) {
  return new Promise((resolve) => {
    if (frameImages[type]) return resolve(frameImages[type]);
    const img = new Image();
    img.onload = () => { frameImages[type] = img; resolve(img); };
    img.onerror = () => resolve(null);
    img.src = `frames/${type}.png`;
  });
}

function hexToRgb01(hex) {
  const m = /^#?([0-9a-f]{6})$/i.exec(String(hex).trim());
  if (!m) return null;
  const n = parseInt(m[1], 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

/// ReplaceHue 着色器的 CPU 移植（与 RitsuLib 生成的 gdshader 逐行对应）。
function replaceHueCanvas(img, target, brightness) {
  const canvas = document.createElement("canvas");
  canvas.width = img.naturalWidth;
  canvas.height = img.naturalHeight;
  const ctx = canvas.getContext("2d");
  ctx.drawImage(img, 0, 0);
  const [tr, tg, tb] = target;
  const LUMA = [0.2126, 0.7152, 0.0722];
  const EPS = 1e-7;
  const targetValue = Math.max(tr, tg, tb);
  const th = [tr / Math.max(targetValue, EPS), tg / Math.max(targetValue, EPS), tb / Math.max(targetValue, EPS)];
  const gain = Math.min(1 / Math.max(th[0] * LUMA[0] + th[1] * LUMA[1] + th[2] * LUMA[2], EPS), 1.12);
  const data = ctx.getImageData(0, 0, canvas.width, canvas.height);
  const px = data.data;
  for (let i = 0; i < px.length; i += 4) {
    const r = px[i] / 255, g = px[i + 1] / 255, b = px[i + 2] / 255;
    const maxRgb = Math.max(r, g, b), minRgb = Math.min(r, g, b);
    const value = maxRgb * brightness;
    const sat = (maxRgb - minRgb) / (maxRgb + EPS);
    px[i]     = Math.min(255, 255 * (value + (tr * value * gain - value) * sat));
    px[i + 1] = Math.min(255, 255 * (value + (tg * value * gain - value) * sat));
    px[i + 2] = Math.min(255, 255 * (value + (tb * value * gain - value) * sat));
  }
  ctx.putImageData(data, 0, 0);
  return canvas.toDataURL();
}

/// 卡池 → 卡框颜色方案。自定义人物池用其主题色（与生成的 PoolFrameMaterial 一致）。
function poolFrameColor(pool) {
  if (pool === "Colorless") return { target: [0.87, 0.87, 0.87], brightness: 1.1 };
  for (const ch of state.project?.characters || []) {
    if (pool === ch.className + "CardPool") {
      const rgb = hexToRgb01(ch.color || "#8080FF");
      if (rgb) return { target: rgb, brightness: 1.0 };
    }
  }
  return null; // 未知池：原框
}

async function frameDataUrl(cardType, pool) {
  const type = FRAME_TYPES[cardType] || "skill";
  const img = await loadFrameImage(type);
  if (!img) return null;
  const scheme = poolFrameColor(pool);
  if (!scheme) return img.src;
  const key = `${type}|${scheme.target.join(",")}|${scheme.brightness}`;
  if (!recolorCache[key]) recolorCache[key] = replaceHueCanvas(img, scheme.target, scheme.brightness);
  return recolorCache[key];
}

async function projectImage(rel) {
  if (!rel || !state.dir) return null;
  const key = state.dir + "|" + rel;
  if (!(key in imageCache)) {
    try {
      imageCache[key] = await invoke("read_image", { dir: state.dir, rel });
    } catch {
      imageCache[key] = null;
    }
  }
  return imageCache[key];
}

/// {Name} / {Name:diff()} 占位符 + BBCode → HTML。resolve(name) 返回数值或 null。
function stsFormat(text, resolve) {
  let s = String(text ?? "");
  s = s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  s = s.replace(/\{([A-Za-z0-9_]+)(:[^}]*)?\}/g, (_, name) => {
    const v = resolve ? resolve(name) : null;
    return v == null
      ? `<span class="ph-unknown" title="未找到数值 ${name}">${name}</span>`
      : `<span class="ph${v.upgraded ? " ph-up" : ""}">${v.text}</span>`;
  });
  s = s.replace(/\[(\/?)([a-z_]+)\]/g, (_, close, tag) => {
    if (STS_COLORS[tag]) return close ? "</span>" : `<span style="color:${STS_COLORS[tag]}">`;
    if (tag === "b" || tag === "i" || tag === "u") return close ? `</${tag}>` : `<${tag}>`;
    if (STS_FX.has(tag)) return close ? "</span>" : `<span class="fx-${tag}">`;
    return "";
  });
  return s.replace(/\n/g, "<br>");
}

/// vars 数组 → resolve 函数（支持升级预览与 Amount 示例值）。
function varResolver(vars, opts = {}) {
  return (name) => {
    if (name === "Amount" && opts.amount != null) return { text: String(opts.amount) };
    for (const v of vars || []) {
      const vn = v.kind === "Power" ? v.power : v.kind;
      if (vn === name) {
        const up = previewShowUpgraded && (v.upgrade || 0) !== 0;
        return { text: String(v.value + (up ? v.upgrade : 0)), upgraded: up };
      }
    }
    return null;
  };
}

function pickText(item, field) {
  return item.text?.zhs?.[field] ?? item.text?.en?.[field] ?? "";
}

// ---------- 卡牌整卡预览 ----------

// 标题缎带横幅（参照游戏截图：银色缎带、两端外翻带 V 形缺口）
const RIBBON_SVG = `<svg viewBox="0 0 600 190" preserveAspectRatio="none">
  <defs>
    <linearGradient id="rb-band" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#f2efe8"/>
      <stop offset="0.55" stop-color="#d9d4c9"/>
      <stop offset="1" stop-color="#b0aa9c"/>
    </linearGradient>
    <linearGradient id="rb-wing" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#c4beb1"/>
      <stop offset="1" stop-color="#8d8678"/>
    </linearGradient>
  </defs>
  <!-- 下垂尾翼（带 V 形缺口） -->
  <polygon points="96,20 12,48 40,92 10,144 96,112" fill="url(#rb-wing)" stroke="#645d4f" stroke-width="3.5" stroke-linejoin="round"/>
  <polygon points="504,20 588,48 560,92 590,144 504,112" fill="url(#rb-wing)" stroke="#645d4f" stroke-width="3.5" stroke-linejoin="round"/>
  <!-- 翻折阴影（横幅端点下方） -->
  <polygon points="78,96 104,112 78,128" fill="#5d5647"/>
  <polygon points="522,96 496,112 522,128" fill="#5d5647"/>
  <!-- 主横幅（微拱） -->
  <path d="M78,8 Q300,-4 522,8 L522,98 Q300,110 78,98 Z" fill="url(#rb-band)" stroke="#7c7566" stroke-width="3.5"/>
</svg>`;

// 费用宝石（红宝石多面体 + 内嵌金圆）
const GEM_SVG = `<svg viewBox="0 0 100 100">
  <defs>
    <radialGradient id="gem-gold" cx="0.42" cy="0.36" r="0.75">
      <stop offset="0" stop-color="#ffe08a"/>
      <stop offset="0.7" stop-color="#f2b93f"/>
      <stop offset="1" stop-color="#c98f22"/>
    </radialGradient>
  </defs>
  <polygon points="50,2 81,13 97,45 89,78 58,98 24,91 4,60 12,23"
    fill="#cf3a2b" stroke="#7c1d14" stroke-width="5" stroke-linejoin="round"/>
  <polygon points="50,2 81,13 66,26 44,22" fill="#ee6a50" opacity="0.85"/>
  <polygon points="4,60 12,23 33,38" fill="#a52619" opacity="0.7"/>
  <polygon points="58,98 89,78 70,64" fill="#a52619" opacity="0.55"/>
  <circle cx="50" cy="52" r="31" fill="url(#gem-gold)" stroke="#7a5715" stroke-width="4"/>
</svg>`;

const CARD_TYPE_LABEL = { Attack: "攻击", Skill: "技能", Power: "能力", Status: "状态", Curse: "诅咒" };

async function renderCardPreview(dock, card) {
  const stage = document.createElement("div");
  stage.className = "cp-stage";
  const frameUrl = await frameDataUrl(card.cardType, card.pool || "Colorless");

  const portraitUrl = await projectImage(card.portrait);
  if (portraitUrl) {
    const art = document.createElement("img");
    art.className = "cp-art";
    art.src = portraitUrl;
    stage.appendChild(art);
  } else {
    const ph = document.createElement("div");
    ph.className = "cp-art cp-art-empty";
    ph.textContent = "无卡图";
    stage.appendChild(ph);
  }

  if (frameUrl) {
    const frame = document.createElement("img");
    frame.className = "cp-frame";
    frame.src = frameUrl;
    stage.appendChild(frame);
  }

  // 标题缎带（游戏里标题在顶部横幅上，深色字）
  const ribbon = document.createElement("div");
  ribbon.className = "cp-ribbon";
  ribbon.innerHTML = RIBBON_SVG;
  stage.appendChild(ribbon);

  const title = document.createElement("div");
  title.className = "cp-title";
  title.textContent = pickText(card, "title") || card.className;
  stage.appendChild(title);

  // 费用宝石（左上角，压在横幅上）
  const gem = document.createElement("div");
  gem.className = "cp-gem";
  gem.innerHTML = GEM_SVG;
  const costNum = document.createElement("span");
  costNum.className = "cp-gem-num";
  costNum.textContent = card.energyCost ?? 1;
  gem.appendChild(costNum);
  stage.appendChild(gem);

  // 类型标签（画面与描述分界线中央的小牌）
  const typeChip = document.createElement("div");
  typeChip.className = "cp-type";
  typeChip.textContent = CARD_TYPE_LABEL[card.cardType] || card.cardType;
  stage.appendChild(typeChip);

  const desc = document.createElement("div");
  desc.className = "cp-desc";
  const inner = document.createElement("div");
  inner.className = "cp-desc-inner";
  inner.innerHTML = stsFormat(pickText(card, "description"), varResolver(card.vars));
  desc.appendChild(inner);
  stage.appendChild(desc);

  dock.appendChild(stage);

  const meta = document.createElement("div");
  meta.className = "cp-meta muted";
  meta.textContent = `${card.cardType} · ${card.rarity} · ${card.pool || "Colorless"}`;
  dock.appendChild(meta);

  // 升级预览开关（有升级数值时才显示）
  if ((card.vars || []).some((v) => (v.upgrade || 0) !== 0)) {
    const line = document.createElement("label");
    line.className = "checkline cp-upgrade";
    const cb = document.createElement("input");
    cb.type = "checkbox";
    cb.checked = previewShowUpgraded;
    cb.onchange = () => { previewShowUpgraded = cb.checked; refreshPreview(); };
    line.append(cb, " 显示升级后数值");
    dock.appendChild(line);
  }
}

// ---------- 悬浮提示样式预览（遗物 / 能力 / 药水） ----------

async function renderTipPreview(dock, kind, item) {
  const tip = document.createElement("div");
  tip.className = "tip-box";

  const head = document.createElement("div");
  head.className = "tip-head";
  const iconRel = item[KINDS[kind].assetField];
  const iconUrl = await projectImage(iconRel);
  if (iconUrl) {
    const icon = document.createElement("img");
    icon.className = "tip-icon";
    icon.src = iconUrl;
    head.appendChild(icon);
  } else {
    const ph = document.createElement("div");
    ph.className = "tip-icon tip-icon-empty";
    ph.textContent = "?";
    head.appendChild(ph);
  }
  const title = document.createElement("div");
  title.className = "tip-title";
  title.textContent = pickText(item, "title") || item.className;
  head.appendChild(title);
  tip.appendChild(head);

  const opts = kind === "powers" ? { amount: 3 } : {};
  const desc = document.createElement("div");
  desc.className = "tip-desc";
  desc.innerHTML = stsFormat(pickText(item, "description"), varResolver(item.vars, opts));
  tip.appendChild(desc);

  if (kind === "powers") {
    const smart = pickText(item, "smartDescription");
    if (smart) {
      const s = document.createElement("div");
      s.className = "tip-desc tip-smart";
      s.innerHTML = stsFormat(smart, varResolver(item.vars, opts));
      tip.appendChild(s);
      const note = document.createElement("div");
      note.className = "muted";
      note.textContent = "↑ 动态描述（示例层数 Amount = 3）";
      tip.appendChild(note);
    }
  }
  if (kind === "relics") {
    const flavor = pickText(item, "flavor");
    if (flavor) {
      const f = document.createElement("div");
      f.className = "tip-flavor";
      f.innerHTML = stsFormat(flavor, null);
      tip.appendChild(f);
    }
  }

  dock.appendChild(tip);
  const meta = document.createElement("div");
  meta.className = "cp-meta muted";
  meta.textContent = kind === "powers"
    ? `${item.powerType} · ${item.stackType}`
    : `${item.rarity || ""} · ${item.pool || ""}`.replace(/^ · | · $/g, "");
  dock.appendChild(meta);
}

// ---------- 停靠与刷新 ----------

let currentDock = null;
let currentPreview = null; // { kind, item }
let previewTimer = 0;

function refreshPreview() {
  if (!currentDock || !currentPreview) return;
  const { kind, item } = currentPreview;
  const dock = currentDock;
  dock.innerHTML = "";
  const h = document.createElement("h4");
  h.className = "cp-head";
  h.textContent = "实时预览";
  dock.appendChild(h);
  const render = kind === "cards"
    ? renderCardPreview(dock, item)
    : renderTipPreview(dock, kind, item);
  render.catch((e) => console.error("preview:", e));
}

function schedulePreview() {
  clearTimeout(previewTimer);
  previewTimer = setTimeout(refreshPreview, 180);
}

/// renderEditor 末尾调用：把编辑器改成 主列 + 预览侧栏 双栏。
function mountPreview(panel, kind, item) {
  if (!PREVIEW_KINDS.has(kind)) {
    currentDock = null;
    currentPreview = null;
    return;
  }
  const main = document.createElement("div");
  main.className = "editor-main";
  while (panel.firstChild) main.appendChild(panel.firstChild);
  const side = document.createElement("div");
  side.className = "editor-side";
  const dock = document.createElement("div");
  dock.className = "preview-dock";
  side.appendChild(dock);
  panel.classList.add("with-preview");
  panel.append(main, side);

  currentDock = dock;
  currentPreview = { kind, item };
  refreshPreview();
  // 输入事件冒泡 → 防抖刷新（值直接写回 item，无需重建编辑器）
  main.addEventListener("input", schedulePreview);
  main.addEventListener("change", schedulePreview);
}
