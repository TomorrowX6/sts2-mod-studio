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
const ACTS = ["Overgrowth", "Hive", "Glory"];
// 怪物招式：无 DynamicVars，数值须填固定值
const MONSTER_OPS = ["damage", "block", "heal", "applyPower", "directDamage", "playSfx", "playVfx", "if", "repeat", "custom"];
// 事件选项：地图事件上下文（无战斗），含事件专用积木
const EVENT_OPS = ["directDamage", "heal", "gainGold", "loseGold", "rewardCards", "rewardPotion", "startCombat", "playSfx", "if", "custom"];

// 每类内容的配置：列表名、字段、触发器白名单、效果积木等
const KINDS = {
  cards: {
    label: "卡牌",
    assetCategory: "cards",
    assetField: "portrait",
    effectsField: "onPlay",
    effectsLabel: "打出效果（按顺序执行）",
    effectOps: ["damage", "directDamage", "block", "heal", "draw", "applyPower", "gainGold", "playSfx", "playVfx", "if", "repeat", "custom"],
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
    effectOps: ["block", "heal", "draw", "applyPower", "gainGold", "playSfx", "playVfx", "if", "repeat", "custom"],
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
    triggers: ["AfterCardDrawn", "AfterOwnerTurnEnd"],
    effectOps: ["applyPower", "block", "heal", "directDamage", "playSfx", "playVfx", "if", "repeat", "custom"],
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
    effectOps: ["directDamage", "block", "heal", "draw", "applyPower", "gainGold", "playSfx", "playVfx", "if", "repeat", "custom"],
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
  monsters: {
    label: "怪物",
    assetCategory: "monsters",
    assetField: "image",
    textFields: ["name"],
    hasVars: false,
    newItem: (n) => ({
      className: "NewMonster" + n, minHp: 15, maxHp: 20,
      moves: [{
        name: "BASIC_ATTACK",
        intents: [{ kind: "attack", amount: 3 }],
        effects: [{ op: "damage", amount: 3 }],
        title: { zhs: "攻击" }, banter: {},
      }],
      text: { zhs: { name: "新怪物" } },
    }),
  },
  encounters: {
    label: "遭遇",
    textFields: ["title", "loss"],
    hasVars: false,
    newItem: (n) => ({
      className: "NewEncounter" + n, acts: ["Glory"], roomType: "Monster", isWeak: false,
      monsters: (state.project?.monsters || []).slice(0, 1).map((m) => ({ monster: m.className })),
      text: { zhs: { title: "新遭遇", loss: "" } },
    }),
  },
  events: {
    label: "事件",
    assetCategory: "events",
    assetField: "image",
    hasVars: true,
    newItem: (n) => ({
      className: "NewEvent" + n, acts: ["Glory"], vars: [],
      pages: [
        {
          key: "INITIAL", description: { zhs: "事件描述。" },
          options: [{ key: "LEAVE", title: { zhs: "离开" }, description: {}, effects: [], goto: "DONE" }],
        },
        { key: "DONE", description: { zhs: "结束描述。" }, options: [] },
      ],
      title: { zhs: "新事件" },
    }),
  },
  characters: {
    label: "人物",
    textFields: ["title", "description"],
    hasVars: false,
    newItem: (n) => ({
      className: "NewCharacter" + n, color: "#8080FF", gender: "Neutral",
      startingHp: 80, startingGold: 99, base: "Ironclad",
      startingDeck: (state.project?.cards || []).slice(0, 1).map((c) => ({ card: c.className, count: 5 })),
      startingRelics: [],
      text: { zhs: { title: "新人物", description: "人物介绍。" } },
    }),
  },
};
const TEXT_FIELD_LABEL = {
  title: "标题", description: "描述", flavor: "风味文本",
  smartDescription: "动态描述({Amount}可用)", name: "名称",
  loss: "死亡文本（{character}/{encounter}可用）",
};

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
      const title = item.text?.zhs?.title || item.text?.zhs?.name || item.title?.zhs;
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
  } else if (kind === "monsters") {
    addField("最小血量", numInput(item.minHp, (v) => item.minHp = v));
    addField("最大血量", numInput(item.maxHp, (v) => item.maxHp = v));
    addField("自定义场景（可空，覆盖内置模板）", textInput(item.scene || "", (v) => item.scene = v.trim() || undefined, "assets/scenes/xxx.tscn"));
  } else if (kind === "encounters") {
    addField("房间类型", select(["Monster", "Elite", "Boss"], item.roomType || "Monster", (v) => item.roomType = v));
    addField("摄像机缩放（可空）", textInput(item.cameraScaling ?? "", (v) => item.cameraScaling = v === "" ? undefined : Number(v), "0.8"));
    const weak = document.createElement("input");
    weak.type = "checkbox";
    weak.checked = !!item.isWeak;
    weak.onchange = () => item.isWeak = weak.checked;
    const weakLine = document.createElement("label");
    weakLine.className = "checkline";
    weakLine.append(weak, " 弱怪池（前几场战斗）");
    grid.appendChild(weakLine);
  } else if (kind === "events") {
    addField("出现条件（C# 布尔表达式，可空）", textInput(item.condition || "", (v) => item.condition = v.trim() || undefined,
      "runState.Players.All(p => p.Gold >= 60)"));
  } else if (kind === "characters") {
    addField("主题色（#RRGGBB）", textInput(item.color || "#8080FF", (v) => item.color = v.trim() || "#8080FF", "#8080FF"));
    addField("性别（人称）", select(["Masculine", "Feminine", "Neutral"], item.gender, (v) => item.gender = v));
    addField("初始血量", numInput(item.startingHp, (v) => item.startingHp = v));
    addField("初始金币", numInput(item.startingGold, (v) => item.startingGold = v));
    addField("资源兜底原版人物", select(["Ironclad", "Silent", "Defect", "Regent", "Necrobinder"], item.base, (v) => item.base = v));
  }

  // 注册到哪些幕（遭遇 / 事件）
  if (kind === "encounters" || kind === "events") {
    const actsRow = document.createElement("div");
    actsRow.className = "row";
    item.acts = item.acts || [];
    for (const act of ACTS) {
      const cb = document.createElement("input");
      cb.type = "checkbox";
      cb.checked = item.acts.includes(act);
      cb.onchange = () => {
        item.acts = cb.checked
          ? [...item.acts, act]
          : item.acts.filter((a) => a !== act);
      };
      const line = document.createElement("label");
      line.className = "checkline";
      line.append(cb, ` ${act}`);
      actsRow.appendChild(line);
    }
    const hint = document.createElement("span");
    hint.className = "muted";
    hint.textContent = kind === "events" ? "（全不勾 = 共享事件）" : "（全不勾 = 全局注册，地图池外）";
    actsRow.appendChild(hint);
    const actsTitle = document.createElement("h4");
    actsTitle.textContent = "出现的幕";
    panel.appendChild(actsTitle);
    panel.appendChild(actsRow);
  }

  // 图片：路径 + 选择按钮
  if (cfg.assetField) {
    panel.appendChild(assetPickRow(item, cfg.assetField, cfg.assetCategory, item.className, "图片（项目内相对路径）"));
  }

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
      item[cfg.effectsField].push(defaultEffect(cfg.effectOps[0]));
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
      addEff.onclick = () => { t.effects.push(defaultEffect(cfg.effectOps[0])); renderEditor(); };
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

  // 各类型专属区块
  if (kind === "monsters") renderMovesSection(panel, item);
  if (kind === "encounters") renderEncounterMonsters(panel, item);
  if (kind === "events") renderEventPages(panel, item);
  if (kind === "characters") renderCharacterExtras(panel, item);

  // 文本
  if (cfg.textFields) {
    panel.appendChild(sectionHeader("文本", null));
    item.text = item.text || {};
    for (const lang of LANGS) {
      for (const field of cfg.textFields) {
        const cur = item.text[lang]?.[field] || "";
        const isLong = field !== "title" && field !== "name";
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
  } else if (kind === "events") {
    panel.appendChild(sectionHeader("事件标题", null));
    langMapInputs(panel, "标题", item, "title");
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

function renderEffectRows(box, effects, allowedOps, opts = {}) {
  box.innerHTML = "";
  effects.forEach((e, i) => {
    const row = document.createElement("div");
    row.className = "row";
    row.appendChild(labeled("动作", select(allowedOps, e.op, (val) => {
      effects[i] = defaultEffect(val);
      renderEditor();
    })));
    if (e.op === "damage" && opts.monster) {
      row.appendChild(labeled("固定伤害", numInput(e.amount ?? 3, (v) => e.amount = v), "narrow"));
    } else if (e.op === "damage") {
      row.appendChild(labeled("数值名", textInput(e.var || "", (v) => e.var = v || undefined, "Damage（默认）")));
    } else if (e.op === "draw") {
      row.appendChild(labeled("数值名", textInput(e.var || "", (v) => e.var = v || undefined, "Cards（默认）")));
    } else if (e.op === "applyPower") {
      row.appendChild(labeled("能力类名", textInput(e.power || "", (v) => e.power = v, "WeakPower")));
      if (!opts.monster) {
        row.appendChild(labeled("数值名/留空", textInput(e.var || "", (v) => e.var = v || undefined)));
      }
      row.appendChild(labeled("固定层数", textInput(e.amount ?? "", (v) => e.amount = v === "" ? undefined : Number(v)), "narrow"));
      const chk = document.createElement("label");
      chk.className = "checkline";
      const cb = document.createElement("input");
      cb.type = "checkbox";
      cb.checked = !!e.toSelf;
      cb.onchange = () => e.toSelf = cb.checked;
      chk.append(cb, "给自己");
      row.appendChild(chk);
    } else if (e.op === "block" || e.op === "heal" || e.op === "gainGold" || e.op === "loseGold") {
      const defName = { block: "Block", heal: "Heal", gainGold: "Gold", loseGold: "Gold" }[e.op];
      if (!opts.monster) {
        row.appendChild(labeled("数值名", textInput(e.var || "", (v) => e.var = v || undefined, defName + "（默认）")));
      }
      row.appendChild(labeled("固定值", textInput(e.amount ?? "", (v) => e.amount = v === "" ? undefined : Number(v)), "narrow"));
    } else if (e.op === "rewardCards") {
      row.appendChild(labeled("可选卡数", numInput(e.count ?? 3, (v) => e.count = v), "narrow"));
    } else if (e.op === "rewardPotion") {
      // 无参数
    } else if (e.op === "startCombat") {
      const encounters = (state.project?.encounters || []).map((x) => x.className);
      if (encounters.length && !e.encounter) e.encounter = encounters[0];
      row.appendChild(labeled("遭遇", encounters.length
        ? select(encounters, e.encounter, (v) => e.encounter = v)
        : textInput(e.encounter || "", (v) => e.encounter = v, "先创建遭遇")));
    } else if (e.op === "directDamage") {
      if (!opts.monster) {
        row.appendChild(labeled("数值名", textInput(e.var || "", (v) => e.var = v || undefined, "Damage（默认）")));
      }
      row.appendChild(labeled("固定值", textInput(e.amount ?? "", (v) => e.amount = v === "" ? undefined : Number(v)), "narrow"));
      if (opts.event) {
        e.toSelf = true; // 事件里只能对玩家自己
      } else {
        row.appendChild(labeled("属性(逗号分隔)", textInput((e.props || []).join(","),
          (v) => e.props = v.split(",").map(s => s.trim()).filter(Boolean), "默认 Unblockable,Unpowered")));
        const chk = document.createElement("label");
        chk.className = "checkline";
        const cb = document.createElement("input");
        cb.type = "checkbox";
        cb.checked = !!e.toSelf;
        cb.onchange = () => e.toSelf = cb.checked;
        chk.append(cb, "对自己");
        row.appendChild(chk);
      }
    } else if (e.op === "playSfx") {
      row.appendChild(labeled("音效事件", textInput(e.event || "", (v) => e.event = v, "event:/sfx/block_gain")));
    } else if (e.op === "playVfx") {
      row.appendChild(labeled("特效路径", textInput(e.path || "", (v) => e.path = v, "vfx/vfx_bloody_impact")));
      const chk = document.createElement("label");
      chk.className = "checkline";
      const cb = document.createElement("input");
      cb.type = "checkbox";
      cb.checked = !!e.onSelf;
      cb.onchange = () => e.onSelf = cb.checked;
      chk.append(cb, "在自己身上");
      row.appendChild(chk);
    } else if (e.op === "if") {
      row.appendChild(labeled("条件（C# 布尔表达式）", textInput(e.when || "", (v) => e.when = v, "Owner.Creature.Block > 0")));
    } else if (e.op === "repeat") {
      row.appendChild(labeled("次数", numInput(e.times ?? 2, (v) => e.times = v), "narrow"));
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

    // if / repeat 的嵌套效果块
    if (e.op === "if") {
      box.appendChild(nestedBlock("满足时", e.then = e.then || [], allowedOps, opts));
      box.appendChild(nestedBlock("否则（可空）", e.else = e.else || [], allowedOps, opts));
    } else if (e.op === "repeat") {
      box.appendChild(nestedBlock("循环体", e.do = e.do || [], allowedOps, opts));
    }
  });
}

function nestedBlock(title, effects, allowedOps, opts = {}) {
  const wrap = document.createElement("div");
  wrap.className = "nested-box";
  const head = document.createElement("div");
  head.className = "nested-head muted";
  head.textContent = title + " ";
  const add = document.createElement("button");
  add.className = "small";
  add.textContent = "＋效果";
  add.onclick = () => { effects.push(defaultEffect(allowedOps[0])); renderEditor(); };
  head.appendChild(add);
  wrap.appendChild(head);
  const inner = document.createElement("div");
  wrap.appendChild(inner);
  renderEffectRows(inner, effects, allowedOps, opts);
  return wrap;
}

function defaultEffect(op) {
  if (op === "applyPower") return { op, power: "WeakPower", toSelf: false };
  if (op === "custom") return { op, code: "" };
  if (op === "playSfx") return { op, event: "" };
  if (op === "playVfx") return { op, path: "", onSelf: false };
  if (op === "if") return { op, when: "", then: [], else: [] };
  if (op === "repeat") return { op, times: 2, do: [] };
  if (op === "rewardCards") return { op, count: 3 };
  if (op === "startCombat") return { op, encounter: state.project?.encounters?.[0]?.className || "" };
  return { op };
}

// ---------- M4 专属区块 ----------

/// 图片路径 + 导入按钮（importName 决定 assets/ 下的文件名）。
function assetPickRow(item, field, category, importName, label) {
  const row = document.createElement("div");
  row.className = "row";
  const input = textInput(item[field] || "", (v) => item[field] = v.trim() || undefined,
    `assets/${category}/${importName}.png`);
  row.appendChild(labeled(label, input));
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
      const rel = await invoke("import_asset", { dir: state.dir, category, className: importName, src });
      item[field] = rel;
      input.value = rel;
      logLine("已导入图片: " + rel);
    } catch (e) { alert("导入失败: " + e); }
  };
  row.appendChild(pick);
  return row;
}

/// 语言 → 文本 的映射输入（owner[key] = { zhs: "…", en: "…" }，空串删除键）。
function langMapInputs(box, label, owner, key, opts = {}) {
  owner[key] = owner[key] || {};
  for (const lang of LANGS) {
    const cur = owner[key][lang] || "";
    const write = (v) => {
      if (v.trim()) owner[key][lang] = v;
      else delete owner[key][lang];
    };
    const el = opts.long ? (() => {
      const ta = document.createElement("textarea");
      ta.rows = 2;
      ta.value = cur;
      ta.oninput = () => write(ta.value);
      return ta;
    })() : textInput(cur, write);
    box.appendChild(labeled(`${label}（${LANG_LABEL[lang]}）`, el));
  }
}

function renderMovesSection(panel, item) {
  panel.appendChild(sectionHeader("招式（按顺序循环出招）", () => {
    item.moves = item.moves || [];
    item.moves.push({
      name: "MOVE_" + (item.moves.length + 1),
      intents: [{ kind: "attack", amount: 3 }],
      effects: [{ op: "damage", amount: 3 }],
      title: {}, banter: {},
    });
    renderEditor();
  }));
  (item.moves = item.moves || []).forEach((mv, mi) => {
    const box = document.createElement("div");
    box.className = "trigger-box";
    const head = document.createElement("div");
    head.className = "row";
    head.appendChild(labeled("招式 ID（大写蛇形）", textInput(mv.name, (v) => mv.name = v.trim(), "BASIC_ATTACK")));
    head.appendChild(delBtn(() => { item.moves.splice(mi, 1); renderEditor(); }));
    box.appendChild(head);

    box.appendChild(sectionHeader("意图（头顶图标，可并列多个）", () => {
      mv.intents = mv.intents || [];
      mv.intents.push({ kind: "attack", amount: 3 });
      renderEditor();
    }));
    (mv.intents = mv.intents || []).forEach((it, ii) => {
      const row = document.createElement("div");
      row.className = "row";
      row.appendChild(labeled("类型", select(["attack", "defend", "custom"], it.kind, (v) => {
        mv.intents[ii] = v === "attack" ? { kind: v, amount: 3 } : v === "custom" ? { kind: v, code: "" } : { kind: v };
        renderEditor();
      })));
      if (it.kind === "attack") row.appendChild(labeled("显示伤害", numInput(it.amount ?? 3, (v) => it.amount = v), "narrow"));
      if (it.kind === "custom") row.appendChild(labeled("C# 表达式", textInput(it.code || "", (v) => it.code = v, "new BuffIntent()")));
      row.appendChild(delBtn(() => { mv.intents.splice(ii, 1); renderEditor(); }));
      box.appendChild(row);
    });

    box.appendChild(sectionHeader("效果（怪物无数值引用，用固定值）", () => {
      mv.effects = mv.effects || [];
      mv.effects.push({ op: "damage", amount: 3 });
      renderEditor();
    }));
    const effBox = document.createElement("div");
    box.appendChild(effBox);
    renderEffectRows(effBox, mv.effects = mv.effects || [], MONSTER_OPS, { monster: true });

    langMapInputs(box, "意图标题", mv, "title");
    langMapInputs(box, "出招对白（可空）", mv, "banter");
    panel.appendChild(box);
  });
}

function renderEncounterMonsters(panel, item) {
  panel.appendChild(sectionHeader("出场怪物（多于一个时自动生成槽位场景）", () => {
    const first = state.project?.monsters?.[0]?.className;
    if (!first) { alert("请先在怪物列表中创建怪物"); return; }
    item.monsters = item.monsters || [];
    item.monsters.push({ monster: first });
    renderEditor();
  }));
  const names = (state.project?.monsters || []).map((m) => m.className);
  (item.monsters = item.monsters || []).forEach((em, i) => {
    const row = document.createElement("div");
    row.className = "row";
    row.appendChild(labeled("怪物", names.length
      ? select(names, em.monster, (v) => em.monster = v)
      : textInput(em.monster || "", (v) => em.monster = v)));
    row.appendChild(labeled("槽位名（可空）", textInput(em.slot || "", (v) => em.slot = v.trim() || undefined, "自动 m" + (i + 1))));
    row.appendChild(delBtn(() => { item.monsters.splice(i, 1); renderEditor(); }));
    panel.appendChild(row);
  });
}

function renderEventPages(panel, item) {
  panel.appendChild(sectionHeader("页面（第一页必须是 INITIAL；没有选项的页面 = 结束页）", () => {
    item.pages = item.pages || [];
    item.pages.push({ key: "PAGE_" + (item.pages.length + 1), description: {}, options: [] });
    renderEditor();
  }));
  const pageKeys = (item.pages || []).map((p) => p.key);
  const NO_GOTO = "（不跳转：选项里需有 startCombat）";
  (item.pages = item.pages || []).forEach((p, pi) => {
    const box = document.createElement("div");
    box.className = "trigger-box";
    const head = document.createElement("div");
    head.className = "row";
    head.appendChild(labeled("页面键（大写蛇形）", textInput(p.key, (v) => p.key = v.trim(), "INITIAL")));
    const addOpt = document.createElement("button");
    addOpt.className = "small";
    addOpt.textContent = "＋选项";
    addOpt.onclick = () => {
      p.options = p.options || [];
      p.options.push({
        key: "OPTION_" + (p.options.length + 1), title: {}, description: {},
        effects: [], goto: pageKeys[pageKeys.length - 1],
      });
      renderEditor();
    };
    head.appendChild(addOpt);
    head.appendChild(delBtn(() => { item.pages.splice(pi, 1); renderEditor(); }));
    box.appendChild(head);
    langMapInputs(box, "页面描述", p, "description", { long: true });

    (p.options = p.options || []).forEach((o, oi) => {
      const ob = document.createElement("div");
      ob.className = "nested-box";
      const oh = document.createElement("div");
      oh.className = "row";
      oh.appendChild(labeled("选项键（大写蛇形）", textInput(o.key, (v) => o.key = v.trim(), "TAKE_DAMAGE")));
      oh.appendChild(labeled("之后跳到页", select([NO_GOTO, ...pageKeys], o.goto || NO_GOTO,
        (v) => o.goto = v === NO_GOTO ? undefined : v)));
      oh.appendChild(delBtn(() => { p.options.splice(oi, 1); renderEditor(); }));
      ob.appendChild(oh);
      langMapInputs(ob, "选项标题", o, "title");
      langMapInputs(ob, "选项描述（可空，{Damage}{Gold}等占位可用）", o, "description");
      ob.appendChild(sectionHeader("选项效果", () => {
        o.effects = o.effects || [];
        o.effects.push(defaultEffect("heal"));
        renderEditor();
      }));
      const effBox = document.createElement("div");
      ob.appendChild(effBox);
      renderEffectRows(effBox, o.effects = o.effects || [], EVENT_OPS, { event: true });
      box.appendChild(ob);
    });
    panel.appendChild(box);
  });
}

function renderCharacterExtras(panel, item) {
  panel.appendChild(sectionHeader("图片资源（未设置的项回退到原版人物）", null));
  const imgs = [
    ["combatImage", "战斗模型图"],
    ["portrait", "头像"],
    ["selectIcon", "选人图标"],
    ["selectIconLocked", "选人锁定图标"],
    ["mapMarker", "地图标记"],
    ["energyIcon", "能量图标 24x24"],
    ["energyIconBig", "能量图标 74x74"],
  ];
  for (const [field, label] of imgs) {
    const importName = item.className + field.charAt(0).toUpperCase() + field.slice(1);
    panel.appendChild(assetPickRow(item, field, "characters", importName, label));
  }

  panel.appendChild(sectionHeader("初始卡组（本项目卡牌）", () => {
    const first = state.project?.cards?.[0]?.className;
    if (!first) { alert("请先创建卡牌"); return; }
    item.startingDeck = item.startingDeck || [];
    item.startingDeck.push({ card: first, count: 1 });
    renderEditor();
  }));
  const cardNames = (state.project?.cards || []).map((c) => c.className);
  (item.startingDeck = item.startingDeck || []).forEach((sc, i) => {
    const row = document.createElement("div");
    row.className = "row";
    row.appendChild(labeled("卡牌", cardNames.length
      ? select(cardNames, sc.card, (v) => sc.card = v)
      : textInput(sc.card, (v) => sc.card = v)));
    row.appendChild(labeled("张数", numInput(sc.count ?? 1, (v) => sc.count = v), "narrow"));
    row.appendChild(delBtn(() => { item.startingDeck.splice(i, 1); renderEditor(); }));
    panel.appendChild(row);
  });

  panel.appendChild(sectionHeader("初始遗物（本项目遗物）", () => {
    const first = state.project?.relics?.[0]?.className;
    if (!first) { alert("请先创建遗物"); return; }
    item.startingRelics = item.startingRelics || [];
    item.startingRelics.push(first);
    renderEditor();
  }));
  const relicNames = (state.project?.relics || []).map((r) => r.className);
  (item.startingRelics = item.startingRelics || []).forEach((r, i) => {
    const row = document.createElement("div");
    row.className = "row";
    row.appendChild(labeled("遗物", relicNames.length
      ? select(relicNames, r, (v) => item.startingRelics[i] = v)
      : textInput(r, (v) => item.startingRelics[i] = v)));
    row.appendChild(delBtn(() => { item.startingRelics.splice(i, 1); renderEditor(); }));
    panel.appendChild(row);
  });

  const hint = document.createElement("p");
  hint.className = "muted";
  hint.textContent = `提示：卡牌/遗物/药水想进入该人物的专属池，把它们的"池"字段填为 ${item.className}CardPool / ${item.className}RelicPool / ${item.className}PotionPool。先古对话（ancients.json）会生成占位文本，发布前建议润色。`;
  panel.appendChild(hint);
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
