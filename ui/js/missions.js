async function loadMissionsFromServer(preserveChecked) {
const prevChecked = new Set(
preserveChecked ? state.missions.filter(m => m.checked).map(m => m.filename) : []
);
try {
const resp = await fetch("/api/missions");
if (!resp.ok) return;
const data = await resp.json();
const rawList = data.missions || [];
state.missions = rawList.map((m, i) => ({
...m,
checked: preserveChecked && prevChecked.size > 0 ? prevChecked.has(m.filename) : (i === 0)
}));
renderMissionSelector();
} catch (_) {
renderMissionSelector();
}
}
function formatMissionDisplayName(filename) {
return String(filename || "")
.replace(/\.blk$/i, "")
.replace(/_/g, " ")
.replace(/\b\w/g, (c) => c.toUpperCase());
}
function formatMissionUnitBadge(unitClass) {
if (!unitClass) return t("config.missionDefaultUnit", "Padrão");
return String(unitClass).replace(/^custom_/, "");
}
function renderMissionSelector() {
const box = document.getElementById("mission-checkbox-list");
if (!box) return;
if (state.missions.length === 0) {
box.innerHTML = `
<div class="mission-empty-card">
<div class="mission-empty-title">${escHtml(t("config.missionEmptyTitle", "Nenhuma missão encontrada"))}</div>
<div class="mission-empty-desc">${escHtml(t("config.noMissionsFound", "Nenhuma missão encontrada na pasta UserMissions do War Thunder."))}</div>
<button type="button" class="mission-folder-btn mission-import-btn" data-empty-import-starters>
${escHtml(t("config.btnImportStarters", "Importar Missões"))}
</button>
</div>
`;
updateMissionCountPill();
return;
}
box.innerHTML = state.missions.map((m, idx) => `
<label class="mission-row-label">
<span class="mission-row-left">
<input type="checkbox" data-mission-idx="${idx}" ${m.checked ? "checked" : ""} />
<span class="mission-filename-text" title="${escHtml(m.filename)}">${escHtml(formatMissionDisplayName(m.filename))}</span>
</span>
<span class="mission-unit-hint font-mono-tech" title="${escHtml(m.current_unit_class)}">${escHtml(formatMissionUnitBadge(m.current_unit_class))}</span>
</label>
`).join("");
updateMissionCountPill();
}
function updateMissionCountPill() {
const cnt = state.missions.filter(m => m.checked).length;
const pill = document.getElementById("btn-mission-count-pill");
if (pill) pill.textContent = `(${cnt})`;
}
async function openUserMissionsFolder() {
const btn = document.getElementById("btn-open-user-missions");
if (btn) btn.disabled = true;
try {
const resp = await fetch("/api/missions/open-folder", { method: "POST" });
const data = await resp.json().catch(() => ({ ok: true }));
if (resp.ok && data.ok) {
showFeedbackBanner({ type: "ok", kind: "folderOpened" }, 4000);
} else {
showFeedbackBanner({ type: "err", rawText: data.error || "Could not open folder." });
}
} catch (err) {
showFeedbackBanner({ type: "err", rawText: err.message });
} finally {
if (btn) btn.disabled = false;
}
}
async function importStarterMissions() {
const btn = document.getElementById("btn-import-starters");
if (btn) btn.disabled = true;
try {
const resp = await fetch("/api/missions/import-starters", { method: "POST" });
const data = await resp.json().catch(() => ({ ok: false }));
if (resp.ok && data.ok) {
await loadMissionsFromServer(true);
showFeedbackBanner({ type: "ok", kind: "starterMissionsImported", count: data.count || 3 }, 4500);
} else {
showFeedbackBanner({ type: "err", rawText: data.error || "Import failed." });
}
} catch (err) {
showFeedbackBanner({ type: "err", rawText: err.message });
} finally {
if (btn) btn.disabled = false;
}
}
async function executeInjection() {
const d = state.activeDetail;
if (!d) return;
const chosenMissions = state.missions.filter(m => m.checked).map(m => m.filename);
if (chosenMissions.length === 0) {
showFeedbackBanner({ type: "err", kind: "selectOneMission" });
return;
}
const btn = document.getElementById("btn-execute-inject");
btn.disabled = true;
const pr = d.presets && d.presets.length > 0 ? (d.presets[state.selectedPresetIndex] || d.presets[0]) : null;
const beltSelect = document.getElementById("cfg-gun-belt-select");
const ammoInput = document.getElementById("cfg-gun-ammo-input");
const belt2Select = document.getElementById("cfg-gun-belt2-select");
const ammo2Input = document.getElementById("cfg-gun-ammo2-input");
const isArmyOrShip = d.branch === "army" || d.branch === "ships";
const rawAmmo = ammoInput ? parseInt(ammoInput.value, 10) : 0;
const effAmmo = isArmyOrShip
? (rawAmmo > 0 ? rawAmmo : (d.ammoCount || 40))
: (rawAmmo > 0 ? rawAmmo : (d.gunAmmoMax || 0));
const rawAmmo2 = ammo2Input ? parseInt(ammo2Input.value, 10) : 0;
const reqBody = {
vehicle_id: d.id,
mode: "native",
preset_id: pr ? pr.id : "",
preset_blk: pr ? (pr.blk || "") : "",
custom_pylons: null,
missions: chosenMissions,
create_backup: false,
bullets0: beltSelect ? beltSelect.value : "",
bullets_count0: effAmmo,
bullets1: belt2Select && !document.getElementById("cfg-tank-slot2-row").classList.contains("is-hidden") ? belt2Select.value : null,
bullets_count1: rawAmmo2 > 0 ? rawAmmo2 : null,
air_spawn_altitude: null,
skin: "default",
};
try {
const resp = await fetch("/api/inject", {
method: "POST",
headers: { "Content-Type": "application/json" },
body: JSON.stringify(reqBody)
});
const result = await resp.json().catch(() => null);
if (!resp.ok || !result || !result.ok) {
const errDetail = (result && (result.detail || result.mission_report?.failed?.[0]?.error)) || `HTTP ${resp.status}`;
throw new Error(errDetail);
}
const succ = result.mission_report?.succeeded || chosenMissions;
showFeedbackBanner({
type: "ok",
kind: "appliedSuccess",
unitClass: result.vehicle_id,
preset: result.weapons || "Default",
appliedList: succ.join(", "),
hasBackup: reqBody.create_backup,
});
await loadMissionsFromServer(true);
} catch (err) {
showFeedbackBanner({ type: "err", rawText: err.message });
} finally {
btn.disabled = false;
}
}