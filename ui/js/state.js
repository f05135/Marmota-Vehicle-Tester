const state = {
vehicles: [],
selectedNations: new Set(["USA"]),
activeBranch: "aviation",
selectedCategory: "ALL",
searchQuery: "",
filters: {
tier: "all",
brMin: "",
brMax: "",
radarOnly: false,
thermalOnly: false,
},
activeWorkspace: "tree",
activeDetail: null,
selectedPresetIndex: 0,
loadoutMode: "native",
customPylons: {},
compareIds: [],
missions: [],
appConfig: null,
theme: "dark",
activeFeedback: null,
modalStatusFeedback: null,
syncPillState: null,
loaderStepKey: "loaderStep1",
isFirstRunActive: false,
lastTelemetryData: null,
};
const BRANCH_CATEGORIES = {
aviation: ["ALL", "FIGHTERS", "BOMBERS", "ATTACK", "HELICOPTERS"],
army: ["ALL", "TANKS", "HEAVY", "TD", "SPAA"],
ships: ["ALL", "BLUEWATER", "COASTAL"],
};
var translations = {};
var fallbackTranslations = null;
var currentLang = "en-US";
var imageObserver = null;
var searchTimeout = null;
var pathDebounce = null;
var feedbackTimer = null;
function switchWorkspace(view) {
state.activeWorkspace = view;
const mainEl = document.getElementById("screen-main");
const cfgEl = document.getElementById("screen-config");
if (mainEl) mainEl.classList.toggle("is-hidden", view !== "tree");
if (cfgEl) cfgEl.classList.toggle("is-hidden", view !== "config");
}
function applyTheme(themeName) {
const norm = String(themeName || "dark").trim().toLowerCase();
const eff = norm === "light" ? "light" : (norm === "auto" || norm === "system" ? "auto" : "dark");
state.theme = eff;
document.documentElement.setAttribute("data-theme", eff);
const icon = document.getElementById("theme-toggle-icon");
if (icon) icon.textContent = eff === "auto" ? "🌓" : (eff === "light" ? "☀️" : "🌙");
const sel = document.getElementById("modal-theme-select");
if (sel) sel.value = eff;
}
function showFeedbackBanner(fbData, autoHideMs = 0) {
if (feedbackTimer) {
clearTimeout(feedbackTimer);
feedbackTimer = null;
}
state.activeFeedback = fbData;
renderActiveFeedbackBanner();
if (autoHideMs > 0) {
feedbackTimer = setTimeout(() => clearFeedbackBanner(), autoHideMs);
}
}
function clearFeedbackBanner() {
if (feedbackTimer) {
clearTimeout(feedbackTimer);
feedbackTimer = null;
}
state.activeFeedback = null;
const banner = document.getElementById("inject-feedback-banner");
if (banner) banner.classList.add("is-hidden");
}
function renderActiveFeedbackBanner() {
const banner = document.getElementById("inject-feedback-banner");
const textEl = document.getElementById("inject-feedback-text");
if (!banner || !textEl) return;
const fb = state.activeFeedback;
if (!fb) {
banner.classList.add("is-hidden");
return;
}
banner.className = `feedback-banner ${fb.type === "err" ? "err" : "ok"}`;
const icon = fb.type === "err" ? ICONS.alertTriangle : ICONS.checkCircle;
if (fb.kind === "gameLaunchSuccess") {
textEl.innerHTML = `${icon} <span><strong>${escHtml(t("gameLaunchSuccess", "War Thunder launch initiated!"))}</strong></span>`;
} else if (fb.kind === "gameLaunchFailed") {
const msg = fb.customMsg || t("gameLaunchFailed", "Could not launch War Thunder automatically.");
textEl.innerHTML = `${icon} <span>${escHtml(msg)}</span>`;
} else if (fb.kind === "selectOneMission") {
textEl.innerHTML = `${icon} <span>${escHtml(t("feedback.selectOneMission", "Select at least one mission."))}</span>`;
} else if (fb.kind === "appliedSuccess") {
const presetLabel = fb.preset || "Default";
textEl.innerHTML = `${icon} <span class="truncate"><strong>${escHtml(fb.unitClass)}.blk</strong> + <strong>${escHtml(presetLabel)}</strong> ${escHtml(t("feedback.appliedSuccess", "applied to:"))} <strong>${escHtml(fb.appliedList)}</strong></span>`;
} else if (fb.kind === "folderOpened") {
textEl.innerHTML = `${icon} <span>${escHtml(t("config.folderOpened", "UserMissions folder opened."))}</span>`;
} else if (fb.kind === "starterMissionsImported") {
const msg = t("config.starterMissionsImported", "{count} starter missions imported!").replace("{count}", fb.count || 0);
textEl.innerHTML = `${icon} <span>${escHtml(msg)}</span>`;
} else if (fb.kind === "customError" || fb.message || fb.rawText) {
textEl.innerHTML = `${icon} <span>${escHtml(fb.rawText || fb.message || "Error")}</span>`;
}
banner.classList.remove("is-hidden");
}
function logToBackend(level, message, extra = null) {
try {
fetch("/api/log", {
method: "POST",
headers: { "Content-Type": "application/json" },
body: JSON.stringify({
level: level || "INFO",
message: String(message),
source: "UI",
extra: extra || null
})
}).catch(() => {});
} catch (_) {}
}