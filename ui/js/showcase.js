async function openVehicleConfig(vehicleId) {
hideWeaponTooltip();
switchWorkspace("config");
clearFeedbackBanner();
document.getElementById("matrix-table-inner").innerHTML = `<div class="empty-state-box">${escHtml(t("config.loadingMatrix", "Loading vehicle weaponry..."))}</div>`;
try {
const resp = await fetch(`/api/vehicle/${encodeURIComponent(vehicleId)}`);
if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
const detail = await resp.json();
state.activeDetail = detail;
state.loadoutMode = "native";
state.customPylons = {};
const firstArmedIdx = (detail.presets || []).findIndex(p => p.group !== "Without load");
state.selectedPresetIndex = firstArmedIdx >= 0 ? firstArmedIdx : 0;
populateShowcaseAndSpecs(detail);
if (!state.missions || state.missions.length === 0) {
loadMissionsFromServer(true);
} else {
renderMissionSelector();
}
renderPylonMatrix();
} catch (err) {
document.getElementById("matrix-table-inner").innerHTML =
`<div class="empty-state-box">Error loading vehicle: ${escHtml(err.message)}</div>`;
}
}
function populateShowcaseAndSpecs(d) {
const isArmy = d.branch === "army";
const isShip = d.branch === "ships";
document.getElementById("cfg-flag-watermark").innerHTML = NATION_FLAGS[d.nation] || "";
const vehCat = d.cat || d.category || "";
document.getElementById("cfg-type-badge").textContent = `${d.nation} • ${t("tabs." + vehCat, vehCat)}`;
document.getElementById("cfg-br-hero").textContent = `${t("config.rank")} ${d.rank} • BR ${d.br}`;
const summary = state.vehicles.find(v => v.id === d.id);
const cleanHeroName = summary ? getLocalizedVehicleName(summary) : cleanVehicleName(d.name);
document.getElementById("cfg-hero-name").textContent = cleanHeroName;
const assetDir = isShip ? "ship" : (isArmy ? "tank" : "aircraft");
const heroImg = `/assets/${assetDir}/${encodeURIComponent(d.id)}.png`;
const heroFallback = isShip
? renderShipSilhouette()
: (isArmy ? renderTankSilhouette() : renderAircraftSilhouette(vehCat));
document.getElementById("cfg-hero-image-box").innerHTML = `
<img src="${heroImg}" alt="" class="showcase-hero-img" onerror="this.style.display='none'; if(this.nextElementSibling) this.nextElementSibling.style.display='flex';" />
<div style="display:none;width:280px;height:160px;align-items:center;justify-content:center;">${heroFallback}</div>
`;
const balBtn = document.getElementById("btn-open-ballistics-lab");
if (balBtn) {
const hasBallistics = (isArmy || isShip) && Array.isArray(d.belts) && d.belts.some(b => b.ballistics && b.ballistics.pen0 > 0);
balBtn.classList.toggle("is-hidden", !hasBallistics);
}
const gunAmmoBox = document.getElementById("cfg-gun-ammo-box");
const row1 = gunAmmoBox ? gunAmmoBox.querySelector(".gun-ammo-row") : null;
const noGunHint = document.getElementById("cfg-no-gun-hint");
const gunAmmoInput = document.getElementById("cfg-gun-ammo-input");
const gunBeltSelect = document.getElementById("cfg-gun-belt-select");
const slot2Row = document.getElementById("cfg-tank-slot2-row");
const gunBelt2Select = document.getElementById("cfg-gun-belt2-select");
const gunAmmo2Input = document.getElementById("cfg-gun-ammo2-input");
if ((isArmy || isShip) && d.is_fcs && (!d.ammoCount || d.ammoCount === 0)) {
if (row1) row1.classList.add("is-hidden");
slot2Row.classList.add("is-hidden");
if (noGunHint) {
noGunHint.textContent = t("specs.vals.none_fcs", "None (Radar / Command Post)");
noGunHint.classList.remove("is-hidden");
}
gunAmmoInput.value = 0;
} else if (isArmy || isShip) {
if (row1) row1.classList.remove("is-hidden");
if (noGunHint) noGunHint.classList.add("is-hidden");
const totalAmmo = d.ammoCount || 40;
gunAmmoInput.value = totalAmmo;
gunAmmoInput.max = totalAmmo;
const belts = d.belts && d.belts.length > 0 ? d.belts : [{ id: "", name: t("config.standardCombatLoadout", "Default") }];
const optsHtml = belts.map(b => {
const pen = b.ballistics && b.ballistics.pen0 > 0 ? ` (${b.ballistics.pen0}mm)` : "";
return `<option value="${escHtml(b.id)}">${escHtml(b.name + pen)}</option>`;
}).join("");
gunBeltSelect.innerHTML = optsHtml;
gunBeltSelect.selectedIndex = 0;
if ((isArmy || isShip) && belts.length > 1) {
slot2Row.classList.remove("is-hidden");
gunBelt2Select.innerHTML = optsHtml;
gunBelt2Select.selectedIndex = 1;
const slot2Default = Math.min(20, Math.max(4, Math.floor(totalAmmo / 3)));
gunAmmo2Input.max = Math.max(1, totalAmmo - 1);
gunAmmo2Input.value = slot2Default;
gunAmmoInput.value = Math.max(1, totalAmmo - slot2Default);
} else {
slot2Row.classList.add("is-hidden");
}
} else if (d.hasGuns) {
if (row1) row1.classList.remove("is-hidden");
if (noGunHint) noGunHint.classList.add("is-hidden");
slot2Row.classList.add("is-hidden");
gunAmmoInput.value = d.gunAmmoMax || 0;
gunAmmoInput.max = d.gunAmmoMax || 99999;
gunBeltSelect.innerHTML = [
{ id: "", name: t("belts.default", "Default") },
{ id: "air_targets", name: t("belts.air_targets", "Air Targets") },
{ id: "ground_targets", name: t("belts.ground_targets", "Ground Targets") },
{ id: "tracers", name: t("belts.tracers", "Tracers") },
{ id: "stealth", name: t("belts.stealth", "Stealth") }
].map(b => `<option value="${escHtml(b.id)}">${escHtml(b.name)}</option>`).join("");
gunBeltSelect.value = "";
} else {
if (row1) row1.classList.add("is-hidden");
slot2Row.classList.add("is-hidden");
if (noGunHint) {
noGunHint.textContent = t("specs.noGun", "No fixed gun");
noGunHint.classList.remove("is-hidden");
}
gunAmmoInput.value = 0;
}
}
function renderPylonMatrix() {
const d = state.activeDetail;
if (!d) return;
if (d.branch === "army" || d.branch === "ships") {
if (d.is_fcs) {
document.getElementById("matrix-table-inner").innerHTML = `
<div style="margin:16px;padding:14px;background:var(--bg-panel);border:1px solid var(--border-main);border-radius:4px;font-size:12px;">
<strong>${escHtml(t("config.fcsPresetTitle"))}</strong><br/>
${escHtml(t("config.fcsBannerNotice"))}
${d.launcher_counterpart ? `<div style="margin-top:10px;"><button type="button" class="wt-btn-metal" data-switch-vehicle="${escHtml(d.launcher_counterpart)}">➜ ${escHtml(t("config.switchToLauncher"))} ${escHtml(d.launcher_counterpart)}</button></div>` : ""}
</div>
`;
return;
}
const ammoSlots = d.ammo_slots || [];
const colHdrCells = ammoSlots.map(s => `<div class="matrix-pylon-hdr-cell font-mono-tech">S${s.slot}</div>`).join("");
const rowCells = ammoSlots.map(s => {
const iconName = s.icon || "apdsfs_tank.png";
const localizedName = s.name_key ? t(s.name_key, s.name || s.code) : (s.name || s.code);
const color = s.type === "sam" ? "#f87171" : (s.type === "heat-fs" ? "#fbbf24" : (s.type === "smk" ? "#94a3b8" : "#38bdf8"));
return `
<div data-tt-ammo-slot="${s.slot}" class="wt-pylon-cell" title="${escHtml(localizedName)}">
<div class="wp-cell-inner">
<img src="/assets/weapon/${encodeURIComponent(iconName)}" alt="" class="wp-cell-img" onerror="this.style.display='none';" />
<span class="wp-cell-code" style="color:${color};">${escHtml(s.code)}</span>
</div>
</div>
`;
}).join("");
const presetList = Array.isArray(d.presets) && d.presets.length > 0
? d.presets
: [{ id: "", name: "Standard Combat Loadout", name_key: "config.standardCombatLoadout" }];
const rowsHtml = presetList.map((pr, idx) => {
const prTitle = pr.name_key ? t(pr.name_key, pr.name) : (pr.name || t("config.standardCombatLoadout"));
const isSel = state.selectedPresetIndex === idx;
return `
<div data-preset-row="${idx}" class="wt-preset-row ${isSel ? "selected" : ""}">
<div class="preset-name-cell">
<div class="preset-title-text truncate" title="${escHtml(prTitle)}">${escHtml(prTitle)}</div>
</div>
<div class="pylon-cells-strip">${rowCells}</div>
</div>
`;
}).join("");
document.getElementById("matrix-table-inner").innerHTML = `
<div class="matrix-col-header-row">
<div class="matrix-name-col-hdr">${escHtml(t("config.presetNameWeapons"))}</div>
<div class="pylon-cells-strip">${colHdrCells}</div>
</div>
<div class="matrix-group-banner">${escHtml(t("groups.To hit ground targets"))}</div>
${rowsHtml}
`;
return;
}
const pylons = Array.isArray(d.pylons) ? d.pylons : [];
const nPylons = pylons.length;
let colHdrCells = "";
for (let p = 1; p <= nPylons; p++) {
colHdrCells += `<div class="matrix-pylon-hdr-cell font-mono-tech">P${p}</div>`;
}
let html = `
<div class="matrix-col-header-row">
<div class="matrix-name-col-hdr">${escHtml(t("config.presetNameWeapons"))}</div>
<div class="pylon-cells-strip">${colHdrCells}</div>
</div>
`;
const allPresets = d.presets || [];
const renderedIndices = new Set();
for (const grp of GROUPS_ORDER) {
const matching = allPresets.map((pr, idx) => ({ pr, idx })).filter(item => item.pr.group === grp);
if (matching.length === 0) continue;
html += `<div class="matrix-group-banner">${escHtml(t(`groups.${grp}`, grp))}</div>`;
for (const { pr, idx } of matching) {
renderedIndices.add(idx);
const isSel = state.selectedPresetIndex === idx;
let rowCells = "";
for (let p = 1; p <= nPylons; p++) {
const opt = pr.cells ? pr.cells[String(p)] : null;
rowCells += `<div data-tt-source="${idx}" data-tt-pylon="${p}" class="wt-pylon-cell">${opt ? renderWeaponCellHtml(opt) : `<span class="empty-dot"></span>`}</div>`;
}
html += `
<div data-preset-row="${idx}" class="wt-preset-row ${isSel ? "selected" : ""}">
<div class="preset-name-cell"><div class="preset-title-text truncate" title="${escHtml(pr.name)}">${escHtml(pr.name)}</div></div>
<div class="pylon-cells-strip">${rowCells}</div>
</div>
`;
}
}
const remaining = allPresets.map((pr, idx) => ({ pr, idx })).filter(item => !renderedIndices.has(item.idx));
if (remaining.length > 0) {
html += `<div class="matrix-group-banner">${escHtml(t("groups.Multi-purpose weapon"))}</div>`;
for (const { pr, idx } of remaining) {
const isSel = state.selectedPresetIndex === idx;
let rowCells = "";
for (let p = 1; p <= nPylons; p++) {
const opt = pr.cells ? pr.cells[String(p)] : null;
rowCells += `<div data-tt-source="${idx}" data-tt-pylon="${p}" class="wt-pylon-cell">${opt ? renderWeaponCellHtml(opt) : `<span class="empty-dot"></span>`}</div>`;
}
html += `
<div data-preset-row="${idx}" class="wt-preset-row ${isSel ? "selected" : ""}">
<div class="preset-name-cell"><div class="preset-title-text truncate">${escHtml(pr.name)}</div></div>
<div class="pylon-cells-strip">${rowCells}</div>
</div>
`;
}
}
document.getElementById("matrix-table-inner").innerHTML = html;
}
function resolveCellOption(sourceAttr, pylonNum) {
const d = state.activeDetail;
if (!d) return null;
const idx = Number(sourceAttr);
const pr = d.presets[idx];
return pr && pr.cells ? (pr.cells[String(pylonNum)] || null) : null;
}
function showWeaponTooltip(opt, pylonNum, clientX, clientY) {
if (!opt) {
hideWeaponTooltip();
return;
}
const tt = document.getElementById("wt-tooltip");
document.getElementById("tt-title").textContent = opt.fullName || opt.name;
const isArmy = state.activeDetail && state.activeDetail.branch === "army";
const prefix = isArmy ? "SLOT " : "P";
document.getElementById("tt-qty").textContent = opt.count > 1 ? `${prefix}${pylonNum} • x${opt.count}` : `${prefix}${pylonNum}`;
document.getElementById("tt-type").textContent = `${t("tooltip.type", "Type")}: ${opt.code}`;
document.getElementById("tt-specs").innerHTML = `
<div>${escHtml(t("tooltip.range", "Range / Pen"))}: <span style="color:#fff;">${escHtml(opt.range)}</span></div>
<div>${escHtml(t("tooltip.performance", "Performance"))}: <span style="color:#fff;">${escHtml(opt.overload)}</span></div>
<div>${escHtml(t("tooltip.unitMass", "Mass"))}: <span style="color:#fff;">${escHtml(opt.mass)}</span></div>
`;
tt.classList.remove("is-hidden");
tt.style.left = `${Math.min(window.innerWidth - 292, clientX + 14)}px`;
tt.style.top = `${Math.max(12, clientY - 118)}px`;
}
function hideWeaponTooltip() {
const tt = document.getElementById("wt-tooltip");
if (tt) tt.classList.add("is-hidden");
}
function openBallisticsLab() {
const modal = document.getElementById("ballistics-modal");
if (!modal || !state.activeDetail) return;
modal.classList.remove("is-hidden");
renderBallisticsTable();
}
function renderBallisticsTable() {
const d = state.activeDetail;
const box = document.getElementById("ballistics-results-table");
if (!d || !box) return;
const dist = Number(document.getElementById("bal-dist-slider").value || 500);
const angleDeg = Number(document.getElementById("bal-angle-slider").value || 0);
document.getElementById("bal-dist-val").textContent = `${dist} m`;
const los100 = Math.round(100 / Math.cos((angleDeg * Math.PI) / 180));
document.getElementById("bal-angle-val").textContent = `${angleDeg}° (${los100} mm LoS)`;
const shells = (d.belts || []).filter(b => b.ballistics && b.ballistics.pen0 > 0);
if (shells.length === 0) {
box.innerHTML = `<div class="empty-state-box">${escHtml(t("ballistics.noData", "No kinetic/chemical shell data available for this vehicle."))}</div>`;
return;
}
const interpPen = (b, m) => {
const pts = [
[0, b.pen0], [100, b.pen100], [500, b.pen500],
[1000, b.pen1000], [1500, b.pen1500], [2000, b.pen2000]
];
if (m <= 0) return pts[0][1];
for (let i = 0; i < pts.length - 1; i++) {
if (m <= pts[i + 1][0]) {
const ratio = (m - pts[i][0]) / (pts[i + 1][0] - pts[i][0]);
return pts[i][1] + ratio * (pts[i + 1][1] - pts[i][1]);
}
}
return Math.max(0, pts[pts.length - 1][1] * (1 - ((m - 2000) / 3000) * 0.25));
};
box.innerHTML = `
<table class="compare-grid-table">
<thead>
<tr>
<th>${escHtml(t("ballistics.colShell", "Shell"))}</th>
<th>${escHtml(t("ballistics.colType", "Type"))}</th>
<th>${escHtml(t("ballistics.colVel", "Vel."))}</th>
<th>${escHtml(t("ballistics.colMass", "Mass"))}</th>
<th>${escHtml(t("ballistics.colTnt", "TNT"))}</th>
<th>${escHtml(t("ballistics.effectivePen", "Effective Pen"))} @ ${dist}m / ${angleDeg}°</th>
</tr>
</thead>
<tbody>
${shells.map(s => {
const b = s.ballistics;
const p0 = interpPen(b, dist);
const slopeMult = 1 - ((1 - (b.pen60deg0 / Math.max(1, b.pen0))) * (angleDeg / 60));
const eff = Math.round(p0 * slopeMult);
return `
<tr>
<td><strong>${escHtml(s.name)}</strong></td>
<td class="font-mono-tech">${escHtml(s.code)}</td>
<td class="font-mono-tech">${b.velocityMs} m/s</td>
<td class="font-mono-tech">${b.massKg} kg</td>
<td class="font-mono-tech">${b.explosiveKg > 0 ? b.explosiveKg + " kg" : "—"}</td>
<td class="font-mono-tech compare-win">${eff} mm</td>
</tr>
`;
}).join("")}
</tbody>
</table>
`;
}