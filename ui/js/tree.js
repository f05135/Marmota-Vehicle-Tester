function clearSearchInput() {
const searchInput = document.getElementById("global-search-input");
if (searchInput && searchInput.value) searchInput.value = "";
state.searchQuery = "";
const clearBtn = document.getElementById("clear-search-btn");
if (clearBtn) clearBtn.classList.add("is-hidden");
}
function getLocalizedVehicleName(v) {
if (v.names && typeof v.names === "object") {
const locName = v.names[currentLang] || v.names["en-US"];
if (locName) return cleanVehicleName(locName);
}
return cleanVehicleName(v.name || v.id);
}
function updateCategoryTabs() {
const cats = BRANCH_CATEGORIES[state.activeBranch] || BRANCH_CATEGORIES.aviation;
const container = document.getElementById("category-tabs");
if (!container) return;
container.style.gridTemplateColumns = `repeat(${cats.length}, minmax(0, 1fr))`;
container.innerHTML = cats.map(cat => {
const activeClass = state.selectedCategory === cat ? "active" : "";
return `<button data-cat="${cat}" class="wt-cat-tab ${activeClass}">${escHtml(t("tabs." + cat, cat))}</button>`;
}).join("");
container.querySelectorAll(".wt-cat-tab").forEach(tab => {
tab.addEventListener("click", () => {
container.querySelectorAll(".wt-cat-tab").forEach(b => b.classList.remove("active"));
tab.classList.add("active");
if (state.selectedCategory !== tab.dataset.cat) {
clearSearchInput();
state.selectedCategory = tab.dataset.cat;
}
renderTechTree();
});
});
}
function setBranch(branch) {
if (state.activeBranch === branch) return;
clearSearchInput();
state.activeBranch = branch;
state.selectedCategory = "ALL";
document.querySelectorAll(".branch-tab").forEach(btn => {
btn.classList.toggle("active", btn.dataset.branch === branch);
});
updateCategoryTabs();
renderTechTree();
}
function matchesSearch(v, rawQuery) {
if (!rawQuery) return true;
const q = rawQuery.trim().toLowerCase();
const norm = s => s.toLowerCase().replace(/[^a-z0-9]/g, "");
const allNames = v.names ? Object.values(v.names).join(" ") : "";
const hay = `${v.id} ${v.name} ${allNames} ${v.nation} ${v.radar || ""} ${v.gun || ""} ${v.folderGroup || ""}`.toLowerCase();
const hayNorm = norm(hay);
const qNorm = norm(q);
if (hay.includes(q) || (qNorm && hayNorm.includes(qNorm))) return true;
const tokens = q.split(/[\s\-_]+/).filter(Boolean);
if (tokens.length > 1 && tokens.every(tok => hay.includes(tok) || hayNorm.includes(norm(tok)))) {
return true;
}
for (const [alias, patterns] of Object.entries(ALIAS_MAP)) {
if ((qNorm.length >= 3 && alias.includes(qNorm)) || qNorm.includes(alias)) {
if (patterns.some(pat => v.id.toLowerCase().includes(pat))) return true;
}
}
return false;
}
function renderNationStrip() {
const strip = document.getElementById("nation-strip");
if (!strip) return;
strip.innerHTML = NATIONS_ORDER.map(nat => {
const isActive = state.selectedNations.has(nat);
return `
<div data-nation="${nat}" class="wt-nation-card ${isActive ? "active" : ""}">
<div class="nation-top-row">
<span class="nation-code-label">${nat}</span>
<button type="button" data-multi-nation="${nat}" title="${escHtml(t("multiNationTooltip", "Toggle multi-nation"))}" class="multi-add-btn ${isActive ? "is-selected" : ""}">
${isActive ? ICONS.checkSmall : ICONS.plusSmall}
</button>
</div>
<div class="nation-flag-frame">${NATION_FLAGS[nat] || ""}</div>
</div>
`;
}).join("");
}
function handleNationToggle(nat, forceMulti, ev) {
clearSearchInput();
const isMulti = forceMulti || (ev && (ev.shiftKey || ev.metaKey || ev.ctrlKey));
if (isMulti) {
if (state.selectedNations.has(nat) && state.selectedNations.size > 1) {
state.selectedNations.delete(nat);
} else {
state.selectedNations.add(nat);
}
} else {
state.selectedNations = new Set([nat]);
}
renderNationStrip();
renderTechTree();
}
function cleanVehicleName(raw) {
if (!raw) return "";
return String(raw)
.replace(/[\uE000-\uF8FF]/g, "")
.replace(/^o\s+/i, "")
.replace(/^[○◊◄►␡␢▃▂▄▅*×•\s]+/, "")
.replace(/\s+/g, " ")
.trim() || raw;
}
function getVehicleSubBranch(v) {
if (v.subBranch) return v.subBranch;
const cat = String(v.cat || v.category || "").toUpperCase();
if (v.branch === "ships") return cat === "COASTAL" ? "coastal" : "bluewater";
if (v.branch === "aviation") return cat === "HELICOPTERS" ? "helicopters" : "aviation";
return v.branch || "army";
}
function renderSubBranchRows(vehicles, showNationFlag, visibleIds) {
const ranksPresent = Array.from(new Set(vehicles.map(v => v.rank))).sort((a, b) => a - b);
return ranksPresent.map(rk => {
const inRank = vehicles.filter(v => v.rank === rk);
const regularCols = [0, 1, 2, 3, 4].map(c => inRank.filter(v => v.col === c));
const premCol = inRank.filter(v => v.col >= 5);
return `
<div class="rank-row-grid">
<div class="rank-badge-col">
<div class="rank-label-small">${escHtml(t("treeHeaderRank", "RANK"))}</div>
<div class="rank-roman-num">${ROMAN_RANKS[rk] || rk}</div>
</div>
<div class="researchable-cols-5">
${regularCols.map(colVehicles => `
<div class="tree-col-stack">
${colVehicles.map((v, idx) => renderVehicleCard(v, showNationFlag, visibleIds, colVehicles, idx)).join("")}
</div>
`).join("")}
</div>
<div class="premium-col-stack">
${premCol.map((v, idx) => renderVehicleCard(v, showNationFlag, visibleIds, premCol, idx)).join("")}
</div>
</div>
`;
}).join("");
}
function renderTechTree() {
const q = state.searchQuery.trim();
const isGlobalSearch = q.length > 0;
const clearBtn = document.getElementById("clear-search-btn");
if (clearBtn) clearBtn.classList.toggle("is-hidden", !isGlobalSearch);
const filtered = state.vehicles.filter(v => {
if (v.branch && v.branch !== state.activeBranch) return false;
const vNat = v.nation === "BRITAIN" ? "UK" : v.nation;
const hasNation = state.selectedNations.has(v.nation) || state.selectedNations.has(vNat);
if (!isGlobalSearch && !hasNation) return false;
const vehCat = v.cat || v.category || "";
if (state.selectedCategory !== "ALL" && vehCat !== state.selectedCategory) return false;
if (isGlobalSearch && !matchesSearch(v, q)) return false;
return true;
});
const container = document.getElementById("tech-tree-scroll");
if (!container) return;
if (filtered.length === 0) {
container.innerHTML = `<div class="empty-state-box">${escHtml(t("emptyFilter", "No vehicles found."))}</div>`;
return;
}
const visibleIds = new Set(filtered.map(v => v.id));
const showNationFlag = isGlobalSearch || state.selectedNations.size > 1;
const subBranchOrder = state.activeBranch === "ships"
? ["bluewater", "coastal"]
: (state.activeBranch === "aviation" ? ["aviation", "helicopters"] : ["army"]);
const groupedBands = subBranchOrder
.map(sb => ({ sb, list: filtered.filter(v => getVehicleSubBranch(v) === sb) }))
.filter(b => b.list.length > 0);
if (groupedBands.length > 1) {
container.innerHTML = groupedBands.map(band => `
<div class="subbranch-band" data-subbranch="${escHtml(band.sb)}">
<div class="subbranch-band-header">
<div class="subbranch-band-tag">${escHtml(t("subBranches." + band.sb, band.sb.toUpperCase()))}</div>
</div>
${renderSubBranchRows(band.list, showNationFlag, visibleIds)}
</div>
`).join("");
} else {
container.innerHTML = renderSubBranchRows(filtered, showNationFlag, visibleIds);
}
if (imageObserver) {
container.querySelectorAll("img[data-src]").forEach(img => imageObserver.observe(img));
} else {
container.querySelectorAll("img[data-src]").forEach(img => {
img.src = img.getAttribute("data-src");
});
}
}
function renderVehicleCard(v, showNationFlag, visibleIds, colStack, stackIdx) {
const slotClass = v.tierType === "premium"
? "wt-slot-premium"
: (v.tierType === "squadron" ? "wt-slot-squadron" : "wt-slot-regular");
const isArmy = v.branch === "army";
const isShip = v.branch === "ships";
const isPremium = v.col >= 5 || v.tierType === "premium" || v.tierType === "squadron";
const hasPredecessorAbove = Boolean(
!isPremium &&
colStack &&
typeof stackIdx === "number" &&
stackIdx > 0 &&
v.prev &&
colStack[stackIdx - 1] &&
colStack[stackIdx - 1].id === v.prev
);
const showArrow = hasPredecessorAbove && visibleIds.has(v.prev) && !state.searchQuery.trim();
const arrowHtml = showArrow ? `<div class="prog-arrow-wrap">${ICONS.arrowDown}</div>` : "";
const cleanName = getLocalizedVehicleName(v);
const assetDir = isShip ? "ship" : (isArmy ? "tank" : "aircraft");
const vehCat = v.cat || v.category || "";
const silhouetteHtml = isShip
? renderShipSilhouette()
: (isArmy ? renderTankSilhouette() : renderAircraftSilhouette(vehCat));
return `
<div>
${arrowHtml}
<article data-vehicle-id="${escHtml(v.id)}" class="wt-slot-card ${slotClass}">
<div class="card-thumb-col">
<div class="card-thumb-img-wrap">
<img data-src="/assets/${assetDir}/${encodeURIComponent(v.id)}.png" alt="" class="card-thumb-img" onerror="this.style.display='none'; if(this.nextElementSibling) this.nextElementSibling.style.display='flex';" />
<div style="display:none;width:100%;height:100%;align-items:center;justify-content:center;">
${silhouetteHtml}
</div>
</div>
<div class="card-br-row">
${showNationFlag ? `<span class="mini-flag-badge">${NATION_FLAGS[v.nation] || ""}</span>` : `<span></span>`}
<span class="card-br-pill font-mono-tech">${escHtml(v.br || "1.0")}</span>
</div>
</div>
<div class="card-info-col">
<div class="card-veh-title" title="${escHtml(cleanName)}">${escHtml(cleanName)}</div>
</div>
</article>
</div>
`;
}