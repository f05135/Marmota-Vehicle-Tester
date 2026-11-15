(function() {
const urlParams = new URLSearchParams(window.location.search);
const urlToken = urlParams.get("token");
if (urlToken) {
try { sessionStorage.setItem("wt_token", urlToken); } catch (_) {}
}
const token = urlToken || (function() {
try { return sessionStorage.getItem("wt_token"); } catch (_) { return null; }
})();
if (token) {
const originalFetch = window.fetch;
window.fetch = function(resource, init) {
const opts = init ? Object.assign({}, init) : {};
opts.headers = opts.headers ? (opts.headers instanceof Headers ? new Headers(opts.headers) : Object.assign({}, opts.headers)) : {};
if (opts.headers instanceof Headers) {
if (!opts.headers.has("X-Session-Token")) opts.headers.set("X-Session-Token", token);
} else if (Array.isArray(opts.headers)) {
opts.headers.push(["X-Session-Token", token]);
} else {
opts.headers["X-Session-Token"] = token;
}
return originalFetch.call(this, resource, opts);
};
}
document.addEventListener("contextmenu", ev => {
ev.preventDefault();
}, { capture: true });
document.addEventListener("keydown", ev => {
const key = String(ev.key || "").toLowerCase();
const isF12 = ev.key === "F12" || ev.keyCode === 123;
const isWinLinuxDevtools = ev.ctrlKey && ev.shiftKey && ["i", "j", "c", "k"].includes(key);
const isMacDevtools = ev.metaKey && ev.altKey && ["i", "j", "c", "u"].includes(key);
const isSourceShortcut = (ev.ctrlKey || ev.metaKey) && ["u", "s"].includes(key);
if (isF12 || isWinLinuxDevtools || isMacDevtools || isSourceShortcut) {
ev.preventDefault();
ev.stopPropagation();
}
}, { capture: true });
})();
function bindEvents() {
const themeBtn = document.getElementById("btn-theme-toggle");
if (themeBtn) {
themeBtn.addEventListener("click", () => {
const next = state.theme === "dark" ? "light" : (state.theme === "light" ? "auto" : "dark");
applyTheme(next);
fetch("/api/config", {
method: "POST",
headers: { "Content-Type": "application/json" },
body: JSON.stringify({ theme: next })
}).catch(() => {});
});
}
if (window.matchMedia) {
const mq = window.matchMedia("(prefers-color-scheme: light)");
if (mq && typeof mq.addEventListener === "function") {
mq.addEventListener("change", () => {
if (state.theme === "auto") applyTheme("auto");
});
}
}
const langSelect = document.getElementById("lang-switch-select");
if (langSelect) {
langSelect.addEventListener("change", async e => {
const newLang = e.target.value;
await loadLocale(newLang);
fetch("/api/config", {
method: "POST",
headers: { "Content-Type": "application/json" },
body: JSON.stringify({ language: newLang })
}).catch(() => {});
});
}
const modalLangSelect = document.getElementById("modal-lang-select");
if (modalLangSelect) {
modalLangSelect.addEventListener("change", async e => {
await loadLocale(e.target.value);
});
}
const modalThemeSelect = document.getElementById("modal-theme-select");
if (modalThemeSelect) {
modalThemeSelect.addEventListener("change", e => applyTheme(e.target.value));
}
document.getElementById("btn-browse-folder").addEventListener("click", triggerBrowseDirectory);
document.getElementById("modal-game-path-input").addEventListener("input", e => handleManualPathInput(e.target.value));
document.getElementById("btn-refresh-vehicles").addEventListener("click", handleRefreshVehicles);
document.getElementById("btn-update-datamine").addEventListener("click", handleUpdateDatamine);
document.getElementById("btn-generate-diagnostic").addEventListener("click", handleGenerateDiagnostic);
document.getElementById("btn-open-logs-folder").addEventListener("click", handleOpenLogsFolder);
document.getElementById("btn-open-config-folder").addEventListener("click", handleOpenConfigFolder);
document.getElementById("btn-launch-game").addEventListener("click", handleLaunchGame);
document.getElementById("btn-open-settings").addEventListener("click", () => openSettingsModal(false));
document.getElementById("btn-close-settings").addEventListener("click", closeSettingsModal);
document.getElementById("btn-save-settings").addEventListener("click", saveSettingsFromModal);
document.getElementById("btn-download-catalog").addEventListener("click", startCatalogDownload);
document.getElementById("btn-retest-perm").addEventListener("click", () => {
runActivePermissionCheck(document.getElementById("modal-game-path-input").value);
});
document.getElementById("btn-open-settings-mac").addEventListener("click", () => {
fetch("/api/open-system-settings", { method: "POST" }).catch(() => {});
});
document.getElementById("btn-close-legal").addEventListener("click", closeLegalModal);
document.getElementById("btn-dismiss-legal").addEventListener("click", closeLegalModal);
[["link-terms-welcome", "terms"], ["link-privacy-welcome", "privacy"], ["link-license-welcome", "license"],
["link-terms-settings", "terms"], ["link-privacy-settings", "privacy"], ["link-license-settings", "license"]]
.forEach(([id, kind]) => {
const el = document.getElementById(id);
if (el) el.addEventListener("click", e => { e.preventDefault(); openLegalModal(kind); });
});
document.querySelectorAll(".branch-tab").forEach(btn => {
btn.addEventListener("click", () => setBranch(btn.dataset.branch));
});
updateCategoryTabs();
const searchInput = document.getElementById("global-search-input");
searchInput.addEventListener("input", e => {
clearTimeout(searchTimeout);
const val = e.target.value;
searchTimeout = setTimeout(() => {
state.searchQuery = val;
renderTechTree();
}, 100);
});
document.getElementById("clear-search-btn").addEventListener("click", () => {
clearSearchInput();
renderTechTree();
});
document.getElementById("nation-strip").addEventListener("click", ev => {
const multiBtn = ev.target.closest("[data-multi-nation]");
if (multiBtn) {
ev.stopPropagation();
handleNationToggle(multiBtn.getAttribute("data-multi-nation"), true, ev);
return;
}
const card = ev.target.closest("[data-nation]");
if (card) handleNationToggle(card.getAttribute("data-nation"), false, ev);
});
document.getElementById("tech-tree-scroll").addEventListener("click", ev => {
const card = ev.target.closest("[data-vehicle-id]");
if (card) openVehicleConfig(card.getAttribute("data-vehicle-id"));
});
document.getElementById("btn-back-to-tree").addEventListener("click", () => {
hideWeaponTooltip();
switchWorkspace("tree");
});
const balOpenBtn = document.getElementById("btn-open-ballistics-lab");
if (balOpenBtn) balOpenBtn.addEventListener("click", openBallisticsLab);
const balCloseBtn = document.getElementById("btn-close-ballistics");
if (balCloseBtn) balCloseBtn.addEventListener("click", () => {
document.getElementById("ballistics-modal").classList.add("is-hidden");
});
const balDist = document.getElementById("bal-dist-slider");
if (balDist) balDist.addEventListener("input", renderBallisticsTable);
const balAngle = document.getElementById("bal-angle-slider");
if (balAngle) balAngle.addEventListener("input", renderBallisticsTable);
document.getElementById("btn-open-user-missions").addEventListener("click", openUserMissionsFolder);
document.getElementById("btn-import-starters").addEventListener("click", importStarterMissions);
document.getElementById("btn-refresh-missions").addEventListener("click", () => loadMissionsFromServer(true));
document.getElementById("btn-toggle-all-missions").addEventListener("click", () => {
const allChecked = state.missions.length > 0 && state.missions.every(m => m.checked);
state.missions.forEach(m => { m.checked = !allChecked; });
renderMissionSelector();
});
const missionListEl = document.getElementById("mission-checkbox-list");
if (missionListEl) {
missionListEl.addEventListener("change", ev => {
const chk = ev.target.closest("[data-mission-idx]");
if (chk) {
const idx = Number(chk.getAttribute("data-mission-idx"));
if (state.missions[idx]) {
state.missions[idx].checked = chk.checked;
updateMissionCountPill();
}
}
});
missionListEl.addEventListener("click", ev => {
const emptyImportBtn = ev.target.closest("[data-empty-import-starters]");
if (emptyImportBtn) {
ev.preventDefault();
importStarterMissions();
}
});
}
document.getElementById("btn-gun-ammo-max").addEventListener("click", () => {
const ammoInput = document.getElementById("cfg-gun-ammo-input");
const ammo2Input = document.getElementById("cfg-gun-ammo2-input");
const slot2Row = document.getElementById("cfg-tank-slot2-row");
if (ammoInput && state.activeDetail) {
const totalCap = state.activeDetail.gunAmmoMax || state.activeDetail.ammoCount || 40;
const hasSlot2 = slot2Row && !slot2Row.classList.contains("is-hidden") && ammo2Input;
if (hasSlot2) {
let s2 = Math.max(0, parseInt(ammo2Input.value || "0", 10) || 0);
if (s2 >= totalCap) {
s2 = Math.floor(totalCap / 2);
ammo2Input.value = s2;
}
ammoInput.value = Math.max(1, totalCap - s2);
} else {
ammoInput.value = totalCap;
}
}
});
["cfg-gun-ammo-input", "cfg-gun-ammo2-input"].forEach(inpId => {
const el = document.getElementById(inpId);
if (!el) return;
el.addEventListener("change", () => {
if (!state.activeDetail) return;
const ammo1 = document.getElementById("cfg-gun-ammo-input");
const ammo2 = document.getElementById("cfg-gun-ammo2-input");
const slot2Row = document.getElementById("cfg-tank-slot2-row");
const totalCap = state.activeDetail.gunAmmoMax || state.activeDetail.ammoCount || 40;
if (slot2Row && !slot2Row.classList.contains("is-hidden") && ammo1 && ammo2) {
let v1 = Math.max(1, parseInt(ammo1.value || "1", 10) || 1);
let v2 = Math.max(1, parseInt(ammo2.value || "1", 10) || 1);
if (v1 + v2 > totalCap) {
if (inpId === "cfg-gun-ammo2-input") {
v2 = Math.min(v2, Math.max(1, totalCap - 1));
v1 = Math.max(1, totalCap - v2);
} else {
v1 = Math.min(v1, Math.max(1, totalCap - 1));
v2 = Math.max(1, totalCap - v1);
}
ammo1.value = v1;
ammo2.value = v2;
}
}
});
});
document.getElementById("btn-execute-inject").addEventListener("click", executeInjection);
document.getElementById("btn-close-feedback").addEventListener("click", clearFeedbackBanner);
const matrixViewport = document.getElementById("matrix-scroll-viewport");
matrixViewport.addEventListener("click", ev => {
const switchBtn = ev.target.closest("[data-switch-vehicle]");
if (switchBtn) {
openVehicleConfig(switchBtn.getAttribute("data-switch-vehicle"));
return;
}
const row = ev.target.closest("[data-preset-row]");
if (row) {
state.loadoutMode = "native";
state.selectedPresetIndex = Number(row.getAttribute("data-preset-row"));
renderPylonMatrix();
}
});
matrixViewport.addEventListener("mousemove", ev => {
const cell = ev.target.closest(".wt-pylon-cell");
if (!cell) { hideWeaponTooltip(); return; }
const ammoSlotNum = cell.getAttribute("data-tt-ammo-slot");
if (ammoSlotNum) {
const slotIdx = Number(ammoSlotNum) - 1;
const s = state.activeDetail && state.activeDetail.ammo_slots ? state.activeDetail.ammo_slots[slotIdx] : null;
if (s) {
const localizedName = s.name_key ? t(s.name_key, s.name || s.code) : (s.name || s.code);
showWeaponTooltip({
fullName: localizedName, name: localizedName, count: 1,
code: s.code || s.type, range: s.range || "Standard",
overload: s.overload || "—", mass: s.mass || "—"
}, ammoSlotNum, ev.clientX, ev.clientY);
return;
}
}
const pNum = Number(cell.getAttribute("data-tt-pylon"));
if (pNum) {
showWeaponTooltip(resolveCellOption(cell.getAttribute("data-tt-source"), pNum), pNum, ev.clientX, ev.clientY);
}
});
matrixViewport.addEventListener("mouseleave", hideWeaponTooltip);
document.addEventListener("keydown", ev => {
if (ev.key === "Escape") {
closeSettingsModal();
closeLegalModal();
hideWeaponTooltip();
const balModal = document.getElementById("ballistics-modal");
if (balModal) balModal.classList.add("is-hidden");
}
});
}
let bootstrapStarted = false;
async function preloadCatalogAndMissions() {
const [treeResp] = await Promise.all([fetch("/api/tree"), loadMissionsFromServer(false)]);
if (treeResp && treeResp.ok) {
const treeData = await treeResp.json();
state.vehicles = treeData.vehicles || [];
renderTechTree();
}
}
async function checkAndSyncDatamineInBackground() {
const pill = document.getElementById("datamine-sync-pill");
const textEl = document.getElementById("datamine-sync-text");
try {
const checkResp = await fetch("/api/datamine/check?auto=true");
if (!checkResp.ok) return;
const checkData = await checkResp.json();
if (!checkData.ok || !checkData.update_available) return;
if (pill && textEl) {
pill.classList.remove("is-hidden", "sync-done");
textEl.textContent = t("datamineUpdatingPill", "Syncing v{0}...").replace("{0}", checkData.remote_version || "");
}
const updResp = await fetch("/api/datamine/update", {
method: "POST",
headers: { "Content-Type": "application/json" },
body: JSON.stringify({ force: false })
});
if (!updResp.ok) { if (pill) pill.classList.add("is-hidden"); return; }
const updData = await updResp.json();
if (updData.ok && updData.updated) {
await preloadCatalogAndMissions();
if (pill && textEl) {
pill.classList.add("sync-done");
textEl.textContent = t("datamineUpdatedPill", "Synced v{0} (+{1})").replace("{0}", updData.version || "").replace("{1}", updData.new_vehicles_count || 0);
setTimeout(() => pill.classList.add("is-hidden"), 6000);
}
} else if (pill) {
pill.classList.add("is-hidden");
}
} catch (_) {
if (pill) pill.classList.add("is-hidden");
}
}
async function bootstrap() {
if (bootstrapStarted) return;
bootstrapStarted = true;
initImageObserver();
renderNationStrip();
bindEvents();
showLoader();
const browserLang = normalizeLanguageCode(navigator.language);
await loadLocale(browserLang);
setLoaderProgress(25, "loaderStep1", t("loaderStep1"));
try {
const cfgResp = await fetch("/api/config");
if (cfgResp.ok) {
state.appConfig = await cfgResp.json();
applyTheme(state.appConfig.theme || "dark");
const initialLang = normalizeLanguageCode(state.appConfig.language || state.appConfig.system_language || browserLang);
if (initialLang !== currentLang) await loadLocale(initialLang);
if (!state.appConfig.first_run_completed || !state.appConfig.assets_installed) {
hideLoader();
openSettingsModal(true);
return;
}
}
setLoaderProgress(65, "loaderStep2", t("loaderStep2"));
await preloadCatalogAndMissions();
setLoaderProgress(100, "loaderStep3", t("loaderStep3"));
setTimeout(() => hideLoader(), 250);
setTimeout(checkAndSyncDatamineInBackground, 2000);
} catch (err) {
hideLoader();
document.getElementById("tech-tree-scroll").innerHTML = `<div class="empty-state-box">Failed to initialize: ${escHtml(err.message)}</div>`;
}
}
if (document.readyState === "loading") {
window.addEventListener("DOMContentLoaded", bootstrap);
} else {
bootstrap();
}