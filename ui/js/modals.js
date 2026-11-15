const setupState = {
step1Ok: false,
step2Ok: false,
step3Ok: false,
isDownloading: false,
downloadPollInterval: null,
permStatusKey: "verifying",
permErrDetail: "",
catalogReadyCount: null,
};
function renderSetupStatusTranslations() {
const permMsg = document.getElementById("perm-message");
if (permMsg) {
if (setupState.permStatusKey === "empty") {
permMsg.textContent = t("modalDetectedWarning", "Warning: Could not detect valid War Thunder folder.");
} else if (setupState.permStatusKey === "ok") {
permMsg.textContent = t("setup.permOk", "Read & write permissions verified OK");
} else if (setupState.permStatusKey === "denied") {
permMsg.textContent = `${t("setup.permDenied", "Write Permission Denied")}: ${setupState.permErrDetail}`;
} else if (setupState.permStatusKey === "verifying") {
permMsg.textContent = t("setup.permVerifying", "Verifying...");
}
}
const readyMsg = document.getElementById("catalog-ready-msg");
if (readyMsg && setupState.catalogReadyCount !== null) {
readyMsg.textContent = t("setup.catalogReady", "Catalog ready ({0} vehicles)").replace("{0}", setupState.catalogReadyCount);
}
}
function evaluateFirstRunReadiness() {
const saveBtn = document.getElementById("btn-save-settings");
if (!state.isFirstRunActive) {
if (saveBtn) {
saveBtn.disabled = false;
saveBtn.style.opacity = "1";
}
return;
}
const updateBadge = (id, ok) => {
const el = document.getElementById(id);
if (!el) return;
el.textContent = ok ? t("setup.badgeComplete", "COMPLETED") : t("setup.badgeRequired", "REQUIRED");
el.style.background = ok ? "rgba(16,185,129,0.2)" : "rgba(245,158,11,0.2)";
el.style.color = ok ? "#34d399" : "#fbbf24";
};
updateBadge("step1-badge", setupState.step1Ok);
updateBadge("step2-badge", setupState.step2Ok);
updateBadge("step3-badge", setupState.step3Ok);
const allReady = setupState.step1Ok && setupState.step2Ok && setupState.step3Ok;
if (saveBtn) {
saveBtn.disabled = !allReady;
saveBtn.style.opacity = allReady ? "1" : "0.5";
}
}
async function runActivePermissionCheck(rawPath) {
const path = (rawPath || "").trim();
const permBox = document.getElementById("perm-status-box");
const permMsg = document.getElementById("perm-message");
const permIcon = document.getElementById("perm-icon");
const actionRow = document.getElementById("perm-action-row");
const btnOpenMac = document.getElementById("btn-open-settings-mac");
if (!path) {
setupState.permStatusKey = "empty";
renderSetupStatusTranslations();
if (permIcon) permIcon.textContent = "⚠";
if (permBox) permBox.style.borderColor = "#f87171";
if (actionRow) actionRow.style.display = "none";
setupState.step2Ok = false;
evaluateFirstRunReadiness();
return;
}
setupState.permStatusKey = "verifying";
renderSetupStatusTranslations();
if (permIcon) permIcon.textContent = "⏳";
try {
const resp = await fetch("/api/check-permissions", {
method: "POST",
headers: { "Content-Type": "application/json" },
body: JSON.stringify({ resources_dir: path })
});
const data = await resp.json();
if (data.permissions_ok) {
setupState.permStatusKey = "ok";
renderSetupStatusTranslations();
if (permIcon) permIcon.textContent = "✓";
if (permBox) permBox.style.borderColor = "#34d399";
if (actionRow) actionRow.style.display = "none";
setupState.step2Ok = true;
} else {
const errDetail = data.error_details || data.message || t("setup.permDenied", "Write Permission Denied");
setupState.permStatusKey = "denied";
setupState.permErrDetail = errDetail;
renderSetupStatusTranslations();
if (permIcon) permIcon.textContent = "⚠";
if (permBox) permBox.style.borderColor = "#f87171";
if (actionRow) actionRow.style.display = "flex";
if (btnOpenMac) btnOpenMac.style.display = data.is_macos ? "inline-block" : "none";
setupState.step2Ok = false;
}
} catch (err) {
setupState.permStatusKey = "error";
if (permMsg) permMsg.textContent = err.message;
if (permIcon) permIcon.textContent = "⚠";
setupState.step2Ok = false;
}
evaluateFirstRunReadiness();
}
async function checkCatalogSetupStatus() {
const readyBox = document.getElementById("catalog-ready-box");
const readyMsg = document.getElementById("catalog-ready-msg");
const dlBox = document.getElementById("catalog-download-box");
try {
const resp = await fetch("/api/setup/status");
if (resp.ok) {
const data = await resp.json();
if (data.assets_installed) {
if (readyBox) readyBox.style.display = "block";
const count = data.total_vehicles || (state.vehicles ? state.vehicles.length : 0);
setupState.catalogReadyCount = count;
if (readyMsg) {
readyMsg.textContent = t("setup.catalogReady", "Catalog ready ({0} vehicles)").replace("{0}", count);
}
if (dlBox) dlBox.style.display = "none";
setupState.step3Ok = true;
} else {
if (readyBox) readyBox.style.display = "none";
if (dlBox) dlBox.style.display = "block";
setupState.step3Ok = false;
}
}
} catch (_) {
setupState.step3Ok = false;
}
evaluateFirstRunReadiness();
}
async function startCatalogDownload() {
if (setupState.isDownloading) return;
setupState.isDownloading = true;
const btn = document.getElementById("btn-download-catalog");
const progBox = document.getElementById("catalog-progress-box");
const progBar = document.getElementById("catalog-progress-bar");
const progMsg = document.getElementById("catalog-progress-msg");
const progPct = document.getElementById("catalog-progress-pct");
const errBox = document.getElementById("catalog-progress-err");
if (btn) btn.disabled = true;
if (progBox) progBox.style.display = "block";
if (errBox) errBox.style.display = "none";
if (progMsg) progMsg.textContent = t("setup.downloadConnecting", "Connecting...");
try {
await fetch("/api/setup/start-download", { method: "POST" });
} catch (e) {
if (errBox) { errBox.textContent = e.message; errBox.style.display = "block"; }
if (btn) btn.disabled = false;
setupState.isDownloading = false;
return;
}
if (setupState.downloadPollInterval) clearInterval(setupState.downloadPollInterval);
setupState.downloadPollInterval = setInterval(async () => {
try {
const resp = await fetch("/api/setup/download-progress");
if (!resp.ok) return;
const p = await resp.json();
if (p.status === "downloading" || p.status === "extracting") {
if (progBar) progBar.style.width = `${p.percent}%`;
if (progPct) progPct.textContent = `${Math.round(p.percent)}%`;
if (progMsg) progMsg.textContent = p.message || t("setup.downloadExtracting");
} else if (p.status === "completed") {
clearInterval(setupState.downloadPollInterval);
setupState.downloadPollInterval = null;
setupState.isDownloading = false;
if (progBar) progBar.style.width = "100%";
if (progPct) progPct.textContent = "100%";
if (progMsg) progMsg.textContent = t("setup.downloadComplete");
setTimeout(() => checkCatalogSetupStatus(), 400);
} else if (p.status === "error") {
clearInterval(setupState.downloadPollInterval);
setupState.downloadPollInterval = null;
setupState.isDownloading = false;
if (errBox) { errBox.textContent = p.error || p.message; errBox.style.display = "block"; }
if (btn) btn.disabled = false;
}
} catch (_) {}
}, 300);
}
function openSettingsModal(isFirstRun = false) {
state.isFirstRunActive = isFirstRun;
const modal = document.getElementById("settings-modal");
const intro = document.getElementById("welcome-intro-block");
const title = document.getElementById("settings-modal-title");
const saveBtn = document.getElementById("btn-save-settings");
const utilRow = document.getElementById("settings-utility-row");
const diagRow = document.getElementById("settings-diagnostic-row");
const closeBtn = document.getElementById("btn-close-settings");
const step1Card = document.getElementById("setup-step1-card");
const step3Card = document.getElementById("setup-step3-card");
const chkLegal = document.getElementById("chk-agree-legal");
const cfg = state.appConfig || {};
if (isFirstRun) {
intro.classList.remove("is-hidden");
step1Card.classList.remove("is-hidden");
step3Card.classList.remove("is-hidden");
utilRow.style.display = "none";
diagRow.style.display = "none";
closeBtn.style.display = "none";
title.textContent = t("welcomeTitle", "Welcome to Marmota Vehicle Tester");
saveBtn.textContent = t("btnContinue", "START MANAGER");
const step2Title = document.getElementById("step2-title-text");
const step2Desc = document.getElementById("step2-desc-text");
if (step2Title) step2Title.textContent = t("setup.step2Header", "Step 2: War Thunder Installation & Permissions");
if (step2Desc) step2Desc.textContent = t("setup.step2Desc");
document.getElementById("step2-badge").style.display = "inline-block";
if (chkLegal) {
setupState.step1Ok = chkLegal.checked;
chkLegal.onchange = () => {
setupState.step1Ok = chkLegal.checked;
evaluateFirstRunReadiness();
};
}
} else {
intro.classList.add("is-hidden");
step1Card.classList.add("is-hidden");
step3Card.classList.add("is-hidden");
utilRow.style.display = "block";
diagRow.style.display = "block";
closeBtn.style.display = "block";
title.textContent = t("settingsModalTitle", "Preferences & Paths");
saveBtn.textContent = t("btnSaveSettings", "SAVE SETTINGS");
const step2Title = document.getElementById("step2-title-text");
const step2Desc = document.getElementById("step2-desc-text");
if (step2Title) step2Title.textContent = t("setup.step2HeaderNormal", "War Thunder Folder");
if (step2Desc) step2Desc.textContent = t("setup.step2DescNormal");
document.getElementById("step2-badge").style.display = "none";
}
document.getElementById("modal-lang-select").value = currentLang;
document.getElementById("modal-theme-select").value = state.theme;
document.getElementById("modal-game-path-input").value = cfg.resources_dir || "";
runActivePermissionCheck(cfg.resources_dir || "");
checkCatalogSetupStatus();
document.getElementById("refresh-status-feedback").style.display = "none";
modal.classList.remove("is-hidden");
}
function refreshModalTranslations() {
const title = document.getElementById("settings-modal-title");
const saveBtn = document.getElementById("btn-save-settings");
const step2Title = document.getElementById("step2-title-text");
const step2Desc = document.getElementById("step2-desc-text");
if (title) title.textContent = state.isFirstRunActive ? t("welcomeTitle") : t("settingsModalTitle");
if (saveBtn) saveBtn.textContent = state.isFirstRunActive ? t("btnContinue") : t("btnSaveSettings");
if (step2Title) step2Title.textContent = state.isFirstRunActive ? t("setup.step2Header") : t("setup.step2HeaderNormal");
if (step2Desc) step2Desc.textContent = state.isFirstRunActive ? t("setup.step2Desc") : t("setup.step2DescNormal");
renderSetupStatusTranslations();
evaluateFirstRunReadiness();
}
function closeSettingsModal() {
if (state.isFirstRunActive) return;
document.getElementById("settings-modal").classList.add("is-hidden");
}
async function triggerBrowseDirectory() {
try {
const resp = await fetch("/api/browse-directory", { method: "POST" });
const data = await resp.json();
if (data.ok && !data.cancelled && data.path) {
document.getElementById("modal-game-path-input").value = data.path;
runActivePermissionCheck(data.path);
}
} catch (err) {
console.warn("Browse directory error:", err);
}
}
function handleManualPathInput(val) {
clearTimeout(pathDebounce);
pathDebounce = setTimeout(() => runActivePermissionCheck(val), 350);
}
async function handleRefreshVehicles() {
const btn = document.getElementById("btn-refresh-vehicles");
const fb = document.getElementById("refresh-status-feedback");
btn.disabled = true;
fb.style.display = "none";
try {
const resp = await fetch("/api/refresh-vehicles", { method: "POST" });
const data = await resp.json();
if (data.ok) {
const treeResp = await fetch("/api/tree");
if (treeResp.ok) {
const treeData = await treeResp.json();
state.vehicles = treeData.vehicles || [];
renderTechTree();
}
fb.textContent = t("refreshSuccess", "Catalog rebuilt ({0} vehicles).").replace("{0}", data.total);
fb.style.color = "var(--accent-green)";
fb.style.display = "block";
}
} catch (err) {
fb.textContent = err.message;
fb.style.color = "#f87171";
fb.style.display = "block";
} finally {
btn.disabled = false;
}
}
async function handleUpdateDatamine() {
const btn = document.getElementById("btn-update-datamine");
const fb = document.getElementById("refresh-status-feedback");
btn.disabled = true;
fb.style.display = "none";
try {
const resp = await fetch("/api/datamine/update", {
method: "POST",
headers: { "Content-Type": "application/json" },
body: JSON.stringify({ force: false })
});
const data = await resp.json();
if (data.ok) {
const treeResp = await fetch("/api/tree");
if (treeResp.ok) {
const treeData = await treeResp.json();
state.vehicles = treeData.vehicles || [];
renderTechTree();
}
const addedAir = data.new_aircraft ? data.new_aircraft.length : 0;
const addedTank = data.new_tanks ? data.new_tanks.length : 0;
fb.textContent = data.updated
? t("datamineUpdatedSuccess").replace("{0}", data.version).replace("{1}", addedAir).replace("{2}", addedTank)
: t("datamineUpToDate").replace("{0}", data.version || "");
fb.style.color = "var(--accent-green)";
fb.style.display = "block";
}
} catch (err) {
fb.textContent = err.message;
fb.style.color = "#f87171";
fb.style.display = "block";
} finally {
btn.disabled = false;
}
}
async function handleOpenLogsFolder() {
await fetch("/api/open-logs-folder", { method: "POST" }).catch(() => {});
}
async function handleOpenConfigFolder() {
await fetch("/api/open-config-folder", { method: "POST" }).catch(() => {});
}
async function handleGenerateDiagnostic() {
const btn = document.getElementById("btn-generate-diagnostic");
const fbBox = document.getElementById("diagnostic-feedback-box");
const fbTitle = document.getElementById("diagnostic-feedback-title");
const fbDesc = document.getElementById("diagnostic-feedback-desc");
btn.disabled = true;
fbBox.style.display = "none";
try {
const resp = await fetch("/api/generate-diagnostic", { method: "POST" });
const data = await resp.json();
fbBox.style.display = "block";
if (data.ok) {
fbTitle.textContent = t("diagnosticSuccessTitle");
fbDesc.textContent = `${data.filename || "wt_diagnostic.zip"} — ${t("diagnosticSuccessDesc")}`;
} else {
fbTitle.textContent = t("diagnosticFailed");
fbDesc.textContent = data.error || data.detail || "Unknown error";
}
} catch (e) {
fbBox.style.display = "block";
fbTitle.textContent = t("diagnosticFailed");
fbDesc.textContent = e.message;
} finally {
btn.disabled = false;
}
}
async function handleLaunchGame() {
const btn = document.getElementById("btn-launch-game");
btn.disabled = true;
try {
const resp = await fetch("/api/launch-game", { method: "POST" });
const data = await resp.json();
if (data.ok) {
showFeedbackBanner({ type: "ok", kind: "gameLaunchSuccess" }, 4500);
} else {
showFeedbackBanner({ type: "err", kind: "gameLaunchFailed", customMsg: data.message || "" }, 6000);
}
} catch (e) {
showFeedbackBanner({ type: "err", kind: "customError", message: e.message }, 6000);
} finally {
setTimeout(() => { btn.disabled = false; }, 1800);
}
}
async function saveSettingsFromModal() {
const chosenLang = document.getElementById("modal-lang-select").value;
const chosenTheme = document.getElementById("modal-theme-select").value;
const chosenPath = document.getElementById("modal-game-path-input").value.trim();
const wasFirstRun = state.isFirstRunActive;
try {
const resp = await fetch("/api/config", {
method: "POST",
headers: { "Content-Type": "application/json" },
body: JSON.stringify({
language: chosenLang,
theme: chosenTheme,
first_run_completed: true,
resources_dir: chosenPath,
})
});
if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
applyTheme(chosenTheme);
if (chosenLang !== currentLang) await loadLocale(chosenLang);
state.isFirstRunActive = false;
document.getElementById("settings-modal").classList.add("is-hidden");
if (wasFirstRun) {
showLoader();
setLoaderProgress(65, "loaderStep2", t("loaderStep2"));
await preloadCatalogAndMissions();
setLoaderProgress(100, "loaderStep3", t("loaderStep3"));
setTimeout(() => hideLoader(), 250);
} else {
await loadMissionsFromServer(true);
}
} catch (e) {
logToBackend("ERROR", `Failed saving settings: ${e.message}`);
}
}
function setLoaderProgress(percent, stepKeyOrMsg, fallbackMsg) {
const bar = document.getElementById("loader-progress-bar");
const txt = document.getElementById("loader-status-text");
const pct = document.getElementById("loader-progress-pct");
if (bar) bar.style.width = `${percent}%`;
if (pct) pct.textContent = `${percent}%`;
if (txt) {
if (stepKeyOrMsg && stepKeyOrMsg.startsWith("loader")) {
state.loaderStepKey = stepKeyOrMsg;
txt.textContent = t(stepKeyOrMsg, fallbackMsg || stepKeyOrMsg);
} else if (stepKeyOrMsg) {
txt.textContent = stepKeyOrMsg;
}
}
}
function showLoader() {
const el = document.getElementById("app-initial-loader");
if (el) el.classList.remove("is-hidden");
}
function hideLoader() {
const el = document.getElementById("app-initial-loader");
if (el) el.classList.add("is-hidden");
}
async function openExternalUrl(url) {
try {
await fetch("/api/open-url", {
method: "POST",
headers: { "Content-Type": "application/json" },
body: JSON.stringify({ url })
});
} catch (_) {
window.open(url, "_blank");
}
}
function openLegalModal(docType) {
const modal = document.getElementById("legal-modal");
const titleEl = document.getElementById("legal-modal-title");
const bodyEl = document.getElementById("legal-modal-body");
if (docType === "terms") {
titleEl.textContent = t("legal.terms", "Terms of Use");
bodyEl.innerHTML = t("legal.termsHtml", "");
} else if (docType === "privacy") {
titleEl.textContent = t("legal.privacy", "Privacy Policy");
bodyEl.innerHTML = t("legal.privacyHtml", "");
} else {
titleEl.textContent = t("legal.license", "GNU GPLv3 License");
bodyEl.innerHTML = t("legal.licenseHtml", "");
}
modal.classList.remove("is-hidden");
}
function closeLegalModal() {
document.getElementById("legal-modal").classList.add("is-hidden");
}