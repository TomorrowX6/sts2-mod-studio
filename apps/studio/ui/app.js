// STS2 Mod Studio 前端。纯 vanilla JS，状态即 project.stsmod.json 的对象树。
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const dialog = window.__TAURI__.dialog;

const state = { dir: null, project: null, selected: -1 };

const $ = (id) => document.getElementById(id);
const VAR_KINDS = ["Damage", "Block", "Cards", "Energy", "Repeat", "Heal", "HpLoss",
  "MaxHp", "Gold", "Stars", "Summon", "Forge", "Power"];

// ---------- 项目打开 / 保存 ----------

async function openProject() {
  const dir = await dialog.open({ directory: true, title: "选择项目文件夹" });
  if (!dir) return;
  try {
    state.project = await invoke("load_project", { dir });
    state.dir = dir;
    state.selected = state.project.cards.length ? 0 : -1;
    renderAll();
  } catch (e) { alert("打开失败: " + e); }
}

async function newProject() {
  const dir = await dialog.open({ directory: true, title: "选择一个空文件夹（文件夹名即 mod id）" });
  if (!dir) return;
  try {
    state.project = await invoke("new_project", { dir });
    state.dir = dir;
    state.selected = 0;
    renderAll();
  } catch (e) { alert("创建失败: " + e); }
}

async function saveProject() {
  if (!state.dir) return;
  collectManifest();
  collectCardScalars();
  try {
    await invoke("save_project", { dir: state.dir, project: state.project });
    logLine("已保存 " + state.dir + "/project.stsmod.json");
  } catch (e) { alert("保存失败: " + e); }
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
  renderCardList();
  renderCard();
}

function renderCardList() {
  const ul = $("card-list");
  ul.innerHTML = "";
  state.project.cards.forEach((c, i) => {
    const li = document.createElement("li");
    li.textContent = c.className + (c.text?.zhs?.title ? `（${c.text.zhs.title}）` : "");
    if (i === state.selected) li.classList.add("selected");
    li.onclick = () => { collectCardScalars(); state.selected = i; renderCardList(); renderCard(); };
    ul.appendChild(li);
  });
}

function card() { return state.selected >= 0 ? state.project.cards[state.selected] : null; }

function renderCard() {
  const c = card();
  $("card-form").classList.toggle("hidden", !c);
  $("no-card").classList.toggle("hidden", !!c);
  $("btn-del-card").disabled = !c;
  if (!c) return;
  $("c-className").value = c.className;
  $("c-pool").value = c.pool || "Colorless";
  $("c-cardType").value = c.cardType;
  $("c-rarity").value = c.rarity;
  $("c-target").value = c.target;
  $("c-energyCost").value = c.energyCost;
  $("c-portrait").value = c.portrait || "";
  $("c-show").checked = !!c.showInLibrary;
  const t = (lang) => (c.text && c.text[lang]) || { title: "", description: "" };
  $("t-zhs-title").value = t("zhs").title;
  $("t-zhs-desc").value = t("zhs").description;
  $("t-en-title").value = t("en").title;
  $("t-en-desc").value = t("en").description;
  renderVars();
  renderEffects();
}

// 数值行：直接改 state 里的对象，删除/新增后重渲染。
function renderVars() {
  const c = card();
  const box = $("vars-rows");
  box.innerHTML = "";
  (c.vars || []).forEach((v, i) => {
    const row = document.createElement("div");
    row.className = "row";
    row.appendChild(labeled("种类", select(VAR_KINDS, v.kind, (val) => { v.kind = val; renderVars(); })));
    if (v.kind === "Power") {
      row.appendChild(labeled("能力类名", textInput(v.power || "", (val) => v.power = val, "StrengthPower")));
    }
    row.appendChild(labeled("数值", numInput(v.value, (val) => v.value = val), "narrow"));
    row.appendChild(labeled("升级+", numInput(v.upgrade || 0, (val) => v.upgrade = val), "narrow"));
    row.appendChild(labeled("属性(逗号分隔)", textInput((v.props || []).join(","),
      (val) => v.props = val.split(",").map(s => s.trim()).filter(Boolean), "Move,Unblockable")));
    row.appendChild(delBtn(() => { c.vars.splice(i, 1); renderVars(); }));
    box.appendChild(row);
  });
}

function renderEffects() {
  const c = card();
  const box = $("effect-rows");
  box.innerHTML = "";
  (c.onPlay || []).forEach((e, i) => {
    const row = document.createElement("div");
    row.className = "row";
    row.appendChild(labeled("动作", select(["damage", "draw", "applyPower", "custom"], e.op, (val) => {
      c.onPlay[i] = defaultEffect(val);
      renderEffects();
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
      cb.type = "checkbox"; cb.checked = !!e.toSelf;
      cb.onchange = () => e.toSelf = cb.checked;
      chk.append(cb, "给自己");
      row.appendChild(chk);
    } else if (e.op === "custom") {
      const ta = document.createElement("textarea");
      ta.rows = 2; ta.value = e.code || "";
      ta.placeholder = "await …;  // 原样插入 OnPlay 的 C# 代码";
      ta.oninput = () => e.code = ta.value;
      const wrap = document.createElement("label");
      wrap.append("C# 代码", ta);
      row.appendChild(wrap);
    }
    row.appendChild(delBtn(() => { c.onPlay.splice(i, 1); renderEffects(); }));
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
  inp.type = "number"; inp.value = value ?? 0;
  inp.oninput = () => onInput(Number(inp.value));
  return inp;
}
function delBtn(onClick) {
  const b = document.createElement("button");
  b.className = "small danger"; b.textContent = "✕"; b.onclick = onClick;
  return b;
}

// ---------- 收集表单 → state ----------

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

function collectCardScalars() {
  const c = card();
  if (!c) return;
  c.className = $("c-className").value.trim();
  c.pool = $("c-pool").value.trim() || "Colorless";
  c.cardType = $("c-cardType").value;
  c.rarity = $("c-rarity").value;
  c.target = $("c-target").value;
  c.energyCost = Number($("c-energyCost").value) || 0;
  const portrait = $("c-portrait").value.trim();
  c.portrait = portrait || undefined;
  c.showInLibrary = $("c-show").checked;
  c.text = c.text || {};
  setText(c, "zhs", $("t-zhs-title").value, $("t-zhs-desc").value);
  setText(c, "en", $("t-en-title").value, $("t-en-desc").value);
}

function setText(c, lang, title, description) {
  if (title.trim() === "" && description.trim() === "") { delete c.text[lang]; return; }
  c.text[lang] = { title, description };
}

// ---------- 卡牌增删 ----------

function addCard() {
  collectCardScalars();
  const n = state.project.cards.length + 1;
  state.project.cards.push({
    className: "NewCard" + n, pool: "Colorless", cardType: "Attack", rarity: "Common",
    target: "AnyEnemy", energyCost: 1, showInLibrary: true,
    vars: [{ kind: "Damage", value: 6, props: ["Move"], upgrade: 3 }],
    onPlay: [{ op: "damage" }],
    text: { zhs: { title: "新卡牌", description: "造成{Damage:diff()}点伤害。" } },
  });
  state.selected = state.project.cards.length - 1;
  renderCardList();
  renderCard();
}

function delCard() {
  if (state.selected < 0) return;
  if (!confirm("删除卡牌 " + card().className + "？")) return;
  state.project.cards.splice(state.selected, 1);
  state.selected = state.project.cards.length ? 0 : -1;
  renderCardList();
  renderCard();
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
  $("btn-add-card").onclick = addCard;
  $("btn-del-card").onclick = delCard;
  $("btn-add-var").onclick = () => { card().vars.push({ kind: "Damage", value: 6, props: ["Move"], upgrade: 0 }); renderVars(); };
  $("btn-add-effect").onclick = () => { card().onPlay.push({ op: "damage" }); renderEffects(); };
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
