// STS2 Mod Studio 前端。编辑面板全动态渲染，输入框直接写回项目对象树。
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const dialog = window.__TAURI__.dialog;

const state = { dir: null, project: null, sel: null }; // sel = { kind, idx }

const $ = (id) => document.getElementById(id);
const VAR_KINDS = ["Damage", "Block", "Cards", "Energy", "Repeat", "Heal", "HpLoss",
  "MaxHp", "Gold", "Stars", "Summon", "Forge", "Power"];
const LANGS = ["zhs", "en"];
const LANG_LABEL = { zhs: "简中", en: "EN" };

// 每类内容的配置：列表名、字段、触发器白名单、效果积木等
const KINDS = {
  cards: {
    label: "卡牌",
    assetCategory: "cards",
    assetField: "portrait",
    effectsField: "onPlay",
    effectsLabel: "打出效果（按顺序执行）",
    effectOps: ["damage", "draw", "applyPower", "custom"],
    textFields: ["title", "description"],
    hasVars: true,
    newItem: (n) => ({
      className: "NewCard" + n, pool: "Colorless", cardType: "Attack", rarity: "Common",
      target: "AnyEnemy", energyCost: 1, showInLibrary: true,
      vars: [{ kind: "Damage", value: 6, props: ["Move"], upgrade: 3 }],
      onPlay: [{ op: "damage" }],
      text: { zhs: { title: "新卡牌", description: "造成{Damage:diff()}点伤害。" } },
    }),
  },
  relics: {
    label: "遗物",
    assetCategory: "relics",
    assetField: "icon",
    triggers: ["AfterPlayerTurnStart"],
    effectOps: ["draw", "applyPower", "custom"],
    textFields: ["title", "description", "flavor"],
    hasVars: true,
    newItem: (n) => ({
      className: "NewRelic" + n, pool: "Shared", rarity: "Common",
      vars: [{ kind: "Cards", value: 1, props: [], upgrade: 0 }],
      triggers: [{ trigger: "AfterPlayerTurnStart", effects: [{ op: "draw" }] }],
      text: { zhs: { title: "新遗物", description: "每回合开始时，抽[blue]{Cards}[/blue]张牌。", flavor: "" } },
    }),
  },
  powers: {
    label: "能力",
    assetCategory: "powers",
    assetField: "icon",
    triggers: ["AfterCardDrawn"],
    effectOps: ["applyPower", "custom"],
    textFields: ["title", "description", "smartDescription"],
    hasVars: false,
    newItem: (n) => ({
      className: "NewPower" + n, powerType: "Buff", stackType: "Counter",
      triggers: [{ trigger: "AfterCardDrawn", effects: [] }],
      text: { zhs: { title: "新能力", description: "效果描述。", smartDescription: "获得[blue]{Amount}[/blue]层效果。" } },
    }),
  },
  potions: {
    label: "药水",
    assetCategory: "potions",
    assetField: "image",
    effectsField: "onUse",
    effectsLabel: "使用效果（按顺序执行）",
    effectOps: ["draw", "applyPower", "custom"],
    textFields: ["title", "description"],
    hasVars: true,
    newItem: (n) => ({
      className: "NewPotion" + n, pool: "Shared", rarity: "Common",
      usage: "CombatOnly", target: "Self",
      vars: [{ kind: "Cards", value: 2, props: [], upgrade: 0 }],
      onUse: [{ op: "draw" }],
      text: { zhs: { title: "新药水", description: "抽{Cards}张牌。" } },
    }),
  },
};
const TEXT_FIELD_LABEL = { title: "标题", description: "描述", flavor: "风味文本", smartDescription: "动态描述({Amount}可用)" };

// ---------- 项目打开 / 保存 ----------

async function openProject() {
  const dir = await dialog.open({ directory: true, title: "选择项目文件夹" });
  if (!dir) return;
  try {
    state.project = await invoke("load_project", { dir });
    state.dir = dir;
    autoSelectFirst();
    renderAll();
  } catch (e) { alert("打开失败: " + e); }
}

async function newProject() {
  const dir = await dialog.open({ directory: true, title: "选择一个空文件夹（文件夹名即 mod id）" });
  if (!dir) return;
  try {
    state.project = await invoke("new_project", { dir });
    state.dir = dir;
    autoSelectFirst();
    renderAll();
  } catch (e) { alert("创建失败: " + e); }
}

function autoSelectFirst() {
  state.sel = null;
  for (const kind of Object.keys(KINDS)) {
    if ((state.project[kind] || []).length) { state.sel = { kind, idx: 0 }; break; }
  }
}

async function saveProject() {
  if (!state.dir) return;
  collectManifest();
  try {
    await invoke("save_project", { dir: state.dir, project: state.project });
    logLine("已保存 " + state.dir + "/project.stsmod.json");
  } catch (e) { alert("保存失败: " + e); }
}

function collectManifest() {
  const m = state.project.manifest;
  m.id = $("m-id").value.trim();
  m.name = $("m-name").value.trim();
  m.author = $("m-author").value.trim();
  m.version = $("m-version").value.trim();
  m.minGameVersion = $("m-minGameVersion").value.trim();
  m.description = $("m-description").value;
  m.affectsGameplay = $("m-affects").checked;
}

// ---------- 渲染 ----------

function renderAll() {
  $("editor").classList.remove("hidden");
  $("pipeline").classList.remove("hidden");
  $("btn-save").disabled = false;
  $("project-path").textContent = state.dir;
  const m = state.project.manifest;
  $("m-id").value = m.id;
  $("m-name").value = m.name;
  $("m-author").value = m.author || "";
  $("m-version").value = m.version;
  $("m-minGameVersion").value = m.minGameVersion;
  $("m-description").value = m.description || "";
  $("m-affects").checked = !!m.affectsGameplay;
  renderLists();
  renderEditor();
}

function renderLists() {
  const box = $("content-lists");
  box.innerHTML = "";
  for (const [kind, cfg] of Object.entries(KINDS)) {
    const h = document.createElement("h3");
    h.textContent = cfg.label + " ";
    const add = document.createElement("button");
    add.className = "small";
    add.textContent = "＋";
    add.onclick = () => {
      state.project[kind] = state.project[kind] || [];
      state.project[kind].push(cfg.newItem(state.project[kind].length + 1));
      state.sel = { kind, idx: state.project[kind].length - 1 };
      renderLists();
      renderEditor();
    };
    h.appendChild(add);
    box.appendChild(h);
    const ul = document.createElement("ul");
    ul.className = "content-list";
    (state.project[kind] || []).forEach((item, idx) => {
      const li = document.createElement("li");
      const title = item.text?.zhs?.title;
      li.textContent = item.className + (title ? `（${title}）` : "");
      if (state.sel && state.sel.kind === kind && state.sel.idx === idx) li.classList.add("selected");
      li.onclick = () => { state.sel = { kind, idx }; renderLists(); renderEditor(); };
      ul.appendChild(li);
    });
    box.appendChild(ul);
  }
}

function selected() {
  if (!state.sel) return null;
  return (state.project[state.sel.kind] || [])[state.sel.idx] || null;
}

function renderEditor() {
  const panel = $("edit-panel");
  panel.innerHTML = "";
  const item = selected();
  if (!item) {
    panel.innerHTML = '<p class="muted">左侧选择或新建内容</p>';
    return;
  }
  const kind = state.sel.kind;
  const cfg = KINDS[kind];

  // 标题栏 + 删除
  const h = document.createElement("h3");
  h.textContent = cfg.label + "编辑 ";
  const del = document.createElement("button");
  del.className = "small danger";
  del.textContent = "删除";
  del.onclick = () => {
    if (!confirm(`删除${cfg.label} ${item.className}？`)) return;
    state.project[kind].splice(state.sel.idx, 1);
    autoSelectFirst();
    renderLists();
    renderEditor();
  };
  h.appendChild(del);
  panel.appendChild(h);

  // 标量字段
  const grid = document.createElement("div");
  grid.className = "grid2";
  panel.appendChild(grid);
  const addField = (label, el) => grid.appendChild(labeled(label, el));

  addField("类名（PascalCase）", textInput(item.className, (v) => { item.className = v.trim(); renderLists(); }));
  if (kind === "cards") {
    addField("卡池", textInput(item.pool || "Colorless", (v) => item.pool = v.trim() || "Colorless", "Colorless 或自定义池类名"));
    addField("类型", select(["Attack", "Skill", "Power", "Status", "Curse"], item.cardType, (v) => item.cardType = v));
    addField("稀有度", select(["Basic", "Common", "Uncommon", "Rare", "Special"], item.rarity, (v) => item.rarity = v));
    addField("目标", select(["AnyEnemy", "AllEnemies", "Self", "None"], item.target, (v) => item.target = v));
    addField("耗能", numInput(item.energyCost, (v) => item.energyCost = v));
    const chk = document.createElement("input");
    chk.type = "checkbox";
    chk.checked = !!item.showInLibrary;
    chk.onchange = () => item.showInLibrary = chk.checked;
    const line = document.createElement("label");
    line.className = "checkline";
    line.append(chk, " 在图鉴中显示");
    grid.appendChild(line);
  } else if (kind === "relics") {
    addField("遗物池", textInput(item.pool || "Shared", (v) => item.pool = v.trim() || "Shared", "Shared 或自定义池类名"));
    addField("稀有度", select(["Common", "Uncommon", "Rare", "Boss", "Shop", "Special"], item.rarity, (v) => item.rarity = v));
  } else if (kind === "powers") {
    addField("类型", select(["Buff", "Debuff"], item.powerType, (v) => item.powerType = v));
    addField("叠加", select(["Counter", "Single"], item.stackType, (v) => item.stackType = v));
  } else if (kind === "potions") {
    addField("药水池", textInput(item.pool || "Shared", (v) => item.pool = v.trim() || "Shared", "Shared 或自定义池类名"));
    addField("稀有度", select(["Common", "Uncommon", "Rare"], item.rarity, (v) => item.rarity = v));
    addField("使用方式", textInput(item.usage || "CombatOnly", (v) => item.usage = v.trim() || "CombatOnly", "CombatOnly"));
    addField("目标", select(["Self", "AnyEnemy", "None"], item.target, (v) => item.target = v));
  }

  // 图片：路径 + 选择按钮
  const assetRow = document.createElement("div");
  assetRow.className = "row";
  const assetInput = textInput(item[cfg.assetField] || "", (v) => item[cfg.assetField] = v.trim() || undefined,
    `assets/${cfg.assetCategory}/${item.className}.png`);
  assetRow.appendChild(labeled("图片（项目内相对路径）", assetInput));
  const pick = document.createElement("button");
  pick.className = "small";
  pick.textContent = "选择图片…";
  pick.onclick = async () => {
    const src = await dialog.open({
      title: "选择图片",
      filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "webp", "svg"] }],
    });
    if (!src) return;
    try {
      const rel = await invoke("import_asset", {
        dir: state.dir, category: cfg.assetCategory, className: item.className, src,
      });
      item[cfg.assetField] = rel;
      assetInput.value = rel;
      logLine("已导入图片: " + rel);
    } catch (e) { alert("导入失败: " + e); }
  };
  assetRow.appendChild(pick);
  panel.appendChild(assetRow);

  // 数值
  if (cfg.hasVars) {
    panel.appendChild(sectionHeader("数值（DynamicVars）", () => {
      item.vars = item.vars || [];
      item.vars.push({ kind: "Damage", value: 6, props: ["Move"], upgrade: 0 });
      renderEditor();
    }));
    const varsBox = document.createElement("div");
    panel.appendChild(varsBox);
    renderVarRows(varsBox, item);
  }

  // 效果（卡牌 onPlay / 药水 onUse）
  if (cfg.effectsField) {
    panel.appendChild(sectionHeader(cfg.effectsLabel, () => {
      item[cfg.effectsField] = item[cfg.effectsField] || [];
      item[cfg.effectsField].push({ op: cfg.effectOps[0] });
      renderEditor();
    }));
    const effBox = document.createElement("div");
    panel.appendChild(effBox);
    renderEffectRows(effBox, item[cfg.effectsField] = item[cfg.effectsField] || [], cfg.effectOps);
  }

  // 触发器（遗物 / 能力）
  if (cfg.triggers) {
    panel.appendChild(sectionHeader("触发器（钩子 + 效果）", () => {
      item.triggers = item.triggers || [];
      item.triggers.push({ trigger: cfg.triggers[0], effects: [] });
      renderEditor();
    }));
    (item.triggers = item.triggers || []).forEach((t, ti) => {
      const box = document.createElement("div");
      box.className = "trigger-box";
      const head = document.createElement("div");
      head.className = "row";
      head.appendChild(labeled("触发时机", select(cfg.triggers, t.trigger, (v) => t.trigger = v)));
      const addEff = document.createElement("button");
      addEff.className = "small";
      addEff.textContent = "＋效果";
      addEff.onclick = () => { t.effects.push({ op: cfg.effectOps[0] }); renderEditor(); };
      head.appendChild(addEff);
      head.appendChild(delBtn(() => { item.triggers.splice(ti, 1); renderEditor(); }));
      box.appendChild(head);
      const effBox = document.createElement("div");
      box.appendChild(effBox);
      renderEffectRows(effBox, t.effects, cfg.effectOps);
      panel.appendChild(box);
    });
    const hint = document.createElement("p");
    hint.className = "muted";
    hint.textContent = "其他钩子暂未收录白名单，可写在下方自定义代码里（完整方法重写）。";
    panel.appendChild(hint);
  }

  // 文本
  panel.appendChild(sectionHeader("文本", null));
  item.text = item.text || {};
  for (const lang of LANGS) {
    for (const field of cfg.textFields) {
      const cur = item.text[lang]?.[field] || "";
      const isLong = field !== "title";
      const el = isLong ? (() => {
        const ta = document.createElement("textarea");
        ta.rows = 2;
        ta.value = cur;
        ta.oninput = () => setTextField(item, lang, field, ta.value, cfg);
        return ta;
      })() : textInput(cur, (v) => setTextField(item, lang, field, v, cfg));
      panel.appendChild(labeled(`${TEXT_FIELD_LABEL[field]}（${LANG_LABEL[lang]}）`, el));
    }
  }
  const tip = document.createElement("p");
  tip.className = "muted";
  tip.textContent = "占位符：{Damage:diff()} {Cards:diff()} 等对应数值名；BBCode：[gold]…[/gold] [blue]…[/blue]";
  panel.appendChild(tip);

  // 自定义代码
  panel.appendChild(sectionHeader("自定义代码（原样插入类体）", null));
  const extra = document.createElement("textarea");
  extra.rows = 4;
  extra.className = "code";
  extra.placeholder = "// 例如重写其他钩子:\n// public override async Task AfterCombatEnd(CombatRoom room) { ... }";
  extra.value = item.extraCode || "";
  extra.oninput = () => item.extraCode = extra.value.trim() ? extra.value : undefined;
  panel.appendChild(extra);
}

function setTextField(item, lang, field, value, cfg) {
  item.text[lang] = item.text[lang] || {};
  item.text[lang][field] = value;
  // 该语言所有字段全空时删掉整个语言项
  if (cfg.textFields.every((f) => !(item.text[lang][f] || "").trim())) {
    delete item.text[lang];
  }
}

function sectionHeader(title, onAdd) {
  const h = document.createElement("h4");
  h.textContent = title + " ";
  if (onAdd) {
    const add = document.createElement("button");
    add.className = "small";
    add.textContent = "＋";
    add.onclick = onAdd;
    h.appendChild(add);
  }
  return h;
}

function renderVarRows(box, item) {
  box.innerHTML = "";
  (item.vars || []).forEach((v, i) => {
    const row = document.createElement("div");
    row.className = "row";
    row.appendChild(labeled("种类", select(VAR_KINDS, v.kind, (val) => { v.kind = val; renderEditor(); })));
    if (v.kind === "Power") {
      row.appendChild(labeled("能力类名", textInput(v.power || "", (val) => v.power = val, "StrengthPower")));
    }
    row.appendChild(labeled("数值", numInput(v.value, (val) => v.value = val), "narrow"));
    row.appendChild(labeled("升级+", numInput(v.upgrade || 0, (val) => v.upgrade = val), "narrow"));
    row.appendChild(labeled("属性(逗号分隔)", textInput((v.props || []).join(","),
      (val) => v.props = val.split(",").map(s => s.trim()).filter(Boolean), "Move,Unblockable")));
    row.appendChild(delBtn(() => { item.vars.splice(i, 1); renderEditor(); }));
    box.appendChild(row);
  });
}

function renderEffectRows(box, effects, allowedOps) {
  box.innerHTML = "";
  effects.forEach((e, i) => {
    const row = document.createElement("div");
    row.className = "row";
    row.appendChild(labeled("动作", select(allowedOps, e.op, (val) => {
      effects[i] = defaultEffect(val);
      renderEditor();
    })));
    if (e.op === "damage") {
      row.appendChild(labeled("数值名", textInput(e.var || "", (v) => e.var = v || undefined, "Damage（默认）")));
    } else if (e.op === "draw") {
      row.appendChild(labeled("数值名", textInput(e.var || "", (v) => e.var = v || undefined, "Cards（默认）")));
    } else if (e.op === "applyPower") {
      row.appendChild(labeled("能力类名", textInput(e.power || "", (v) => e.power = v, "WeakPower")));
      row.appendChild(labeled("数值名/留空", textInput(e.var || "", (v) => e.var = v || undefined)));
      row.appendChild(labeled("固定层数", textInput(e.amount ?? "", (v) => e.amount = v === "" ? undefined : Number(v)), "narrow"));
      const chk = document.createElement("label");
      chk.className = "checkline";
      const cb = document.createElement("input");
      cb.type = "checkbox";
      cb.checked = !!e.toSelf;
      cb.onchange = () => e.toSelf = cb.checked;
      chk.append(cb, "给自己");
      row.appendChild(chk);
    } else if (e.op === "custom") {
      const ta = document.createElement("textarea");
      ta.rows = 2;
      ta.className = "code";
      ta.value = e.code || "";
      ta.placeholder = "await …;  // 原样插入方法体的 C# 代码";
      ta.oninput = () => e.code = ta.value;
      const wrap = document.createElement("label");
      wrap.append("C# 代码", ta);
      row.appendChild(wrap);
    }
    row.appendChild(delBtn(() => { effects.splice(i, 1); renderEditor(); }));
    box.appendChild(row);
  });
}

function defaultEffect(op) {
  if (op === "applyPower") return { op, power: "WeakPower", toSelf: false };
  if (op === "custom") return { op, code: "" };
  return { op };
}

// ---------- DOM 小工具 ----------

function labeled(text, el, cls) {
  const label = document.createElement("label");
  if (cls) label.classList.add(cls);
  label.append(text, el);
  return label;
}
function select(options, value, onChange) {
  const sel = document.createElement("select");
  for (const o of options) sel.add(new Option(o, o));
  sel.value = value;
  sel.onchange = () => onChange(sel.value);
  return sel;
}
function textInput(value, onInput, placeholder) {
  const inp = document.createElement("input");
  inp.value = value ?? "";
  if (placeholder) inp.placeholder = placeholder;
  inp.oninput = () => onInput(inp.value);
  return inp;
}
function numInput(value, onInput) {
  const inp = document.createElement("input");
  inp.type = "number";
  inp.value = value ?? 0;
  inp.oninput = () => onInput(Number(inp.value));
  return inp;
}
function delBtn(onClick) {
  const b = document.createElement("button");
  b.className = "small danger";
  b.textContent = "✕";
  b.onclick = onClick;
  return b;
}

// ---------- 流水线 ----------

function logLine(s) {
  const pre = $("log");
  pre.textContent += s + "\n";
  pre.scrollTop = pre.scrollHeight;
}

async function runStep(step) {
  if (!state.dir) return;
  await saveProject();
  $("busy").classList.remove("hidden");
  for (const id of ["btn-generate", "btn-build", "btn-pack", "btn-deploy"]) $(id).disabled = true;
  try {
    await invoke("run_step", { dir: state.dir, step });
    logLine("=== " + step + " 完成 ===");
  } catch (e) {
    logLine("=== 失败: " + e + " ===");
  } finally {
    $("busy").classList.add("hidden");
    for (const id of ["btn-generate", "btn-build", "btn-pack", "btn-deploy"]) $(id).disabled = false;
  }
}

// ---------- 设置 ----------

async function loadConfig() {
  const cfg = await invoke("get_config");
  $("cfg-sts2Dir").value = cfg.sts2Dir || "";
  $("cfg-godotExe").value = cfg.godotExe || "";
  $("cfg-dotnet").value = cfg.dotnet || "";
  $("cfg-pckArch").value = cfg.pckArch || "";
}

async function saveConfig() {
  const cfg = {
    sts2Dir: $("cfg-sts2Dir").value.trim() || null,
    godotExe: $("cfg-godotExe").value.trim() || null,
    dotnet: $("cfg-dotnet").value.trim() || null,
    pckArch: $("cfg-pckArch").value || null,
  };
  await invoke("set_config", { cfg });
  logLine("配置已保存");
}

async function runDoctor() {
  const checks = await invoke("doctor", { dir: state.dir });
  const ul = $("doctor-result");
  ul.innerHTML = "";
  for (const c of checks) {
    const li = document.createElement("li");
    li.className = c.ok ? "ok" : "bad";
    li.textContent = `${c.name}: ${c.detail}`;
    ul.appendChild(li);
  }
}

// ---------- 绑定 ----------

window.addEventListener("DOMContentLoaded", () => {
  $("btn-open").onclick = openProject;
  $("btn-new").onclick = newProject;
  $("btn-save").onclick = saveProject;
  $("btn-generate").onclick = () => runStep("generate");
  $("btn-build").onclick = () => runStep("build");
  $("btn-pack").onclick = () => runStep("pack");
  $("btn-deploy").onclick = () => runStep("deploy");
  $("btn-settings").onclick = () => $("settings").classList.toggle("hidden");
  $("btn-save-config").onclick = saveConfig;
  $("btn-doctor").onclick = runDoctor;
  listen("pipeline-log", (ev) => logLine(ev.payload));
  loadConfig();
});
