const SUPPORTED_LOCALES={"en-US":{file:"en.json",short:"EN",name:"English"},"pt-BR":{file:"pt.json",short:"PT",name:"Português (Brasil)"},"es":{file:"es.json",short:"ES",name:"Español"},"ru":{file:"ru.json",short:"RU",name:"Русский"},"fr":{file:"fr.json",short:"FR",name:"Français"},"zh":{file:"zh.json",short:"ZH",name:"中文 (简体)"},"ja":{file:"ja.json",short:"JA",name:"日本語"}};
function normalizeLanguageCode(code){
if(!code)return "en-US";
if(SUPPORTED_LOCALES[code])return code;
const lower=String(code).toLowerCase().replace("_","-");
if(lower.startsWith("pt"))return "pt-BR";
if(lower.startsWith("es"))return "es";
if(lower.startsWith("ru"))return "ru";
if(lower.startsWith("fr"))return "fr";
if(lower.startsWith("zh"))return "zh";
if(lower.startsWith("ja")||lower.startsWith("jp"))return "ja";
return "en-US";
}
function t(path, fallback = "") {
const parts = path.split(".");
let curr = translations;
for (const p of parts) {
if (!curr || typeof curr !== "object" || !(p in curr)) {
curr = null;
break;
}
curr = curr[p];
}
if (typeof curr === "string") return curr;
if (translations !== fallbackTranslations && fallbackTranslations) {
let fCurr = fallbackTranslations;
for (const p of parts) {
if (!fCurr || typeof fCurr !== "object" || !(p in fCurr)) {
fCurr = null;
break;
}
fCurr = fCurr[p];
}
if (typeof fCurr === "string") return fCurr;
}
return fallback || path;
}
async function loadLocale(lang) {
currentLang = normalizeLanguageCode(lang);
const loc = SUPPORTED_LOCALES[currentLang] || SUPPORTED_LOCALES["en-US"];
try {
if (!fallbackTranslations && loc.file !== "en.json") {
const fbResp = await fetch("/ui/locales/en.json");
if (fbResp.ok) fallbackTranslations = await fbResp.json();
}
const resp = await fetch(`/ui/locales/${loc.file}`);
if (resp.ok) {
translations = await resp.json();
if (loc.file === "en.json") fallbackTranslations = translations;
}
} catch (_) {
translations = fallbackTranslations || {};
}
document.documentElement.setAttribute("lang", currentLang.slice(0, 2));
document.querySelectorAll("[data-i18n]").forEach(el => {
const key = el.getAttribute("data-i18n");
const trans = t(key);
if (trans) el.textContent = trans;
});
document.querySelectorAll("[data-i18n-placeholder]").forEach(el => {
const key = el.getAttribute("data-i18n-placeholder");
const trans = t(key);
if (trans) el.setAttribute("placeholder", trans);
});
document.querySelectorAll("[data-i18n-title]").forEach(el => {
const key = el.getAttribute("data-i18n-title");
const trans = t(key);
if (trans) el.setAttribute("title", trans);
});
const langSelect = document.getElementById("lang-switch-select");
if (langSelect) langSelect.value = currentLang;
const modalLangSelect = document.getElementById("modal-lang-select");
if (modalLangSelect) modalLangSelect.value = currentLang;
updateCategoryTabs();
if (state.vehicles && state.vehicles.length > 0) {
renderTechTree();
}
if (state.activeDetail) {
populateShowcaseAndSpecs(state.activeDetail);
renderPylonMatrix();
updateShowcaseLoadoutSummary();
}
if (typeof renderActiveFeedbackBanner === "function") renderActiveFeedbackBanner();
if (typeof refreshModalTranslations === "function") refreshModalTranslations();
if (typeof renderComparator === "function") renderComparator(true);
if (typeof renderMissionsManager === "function") renderMissionsManager();
if (typeof renderInjectionHistory === "function") renderInjectionHistory();
if (typeof renderTelemetryHUD === "function") renderTelemetryHUD(state.lastTelemetryData);
if (typeof renderBallisticsTable === "function") renderBallisticsTable();
if (typeof renderEfficiencyModal === "function" && !document.getElementById("efficiency-modal")?.classList.contains("is-hidden")) {
renderEfficiencyModal();
}
if (state.loaderStepKey) {
const loaderTxt = document.getElementById("loader-status-text");
if (loaderTxt) loaderTxt.textContent = t(state.loaderStepKey);
}
}