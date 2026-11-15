"use strict";
const ICONS = {
diamond: `<svg viewBox="0 0 10 10" class="svg-xs" style="display:inline-block;" fill="currentColor"><polygon points="5,0.8 9.2,5 5,9.2 0.8,5"/></svg>`,
arrowDown: `<svg viewBox="0 0 14 14" class="svg-sm" style="color:#d9442c;" fill="currentColor"><path d="M6 1h2v7h3L7 13 3 8h3V1z"/></svg>`,
radar: `<svg viewBox="0 0 16 16" class="svg-sm" style="color:#38bdf8;" fill="none" stroke="currentColor" stroke-width="1.7"><path d="M3 13a7 7 0 0 1 10-10M5.5 10.5a3.5 3.5 0 0 1 5-5"/><circle cx="8" cy="8" r="1.3" fill="currentColor"/></svg>`,
rwr: `<svg viewBox="0 0 16 16" class="svg-sm" style="color:#f59e0b;" fill="none" stroke="currentColor" stroke-width="1.7"><circle cx="8" cy="8" r="6"/><path d="M8 4.5v4M8 11v.5" stroke-linecap="round"/></svg>`,
hmd: `<svg viewBox="0 0 16 16" class="svg-sm" style="color:#34d399;" fill="none" stroke="currentColor" stroke-width="1.6"><rect x="2.5" y="4.5" width="11" height="7" rx="2"/><path d="M6 8h4M8 6v4" stroke-linecap="round"/></svg>`,
ccip: `<svg viewBox="0 0 16 16" class="svg-sm" style="color:#f87171;" fill="none" stroke="currentColor" stroke-width="1.6"><circle cx="8" cy="8" r="4.8"/><path d="M8 1.5v2.5M8 12v2.5M1.5 8H4M12 8h2.5" stroke-linecap="round"/></svg>`,
gun: `<svg viewBox="0 0 16 16" class="svg-sm" style="color:#94a3b8;" fill="none" stroke="currentColor" stroke-width="1.7"><path d="M2 9h8l4-2v2l-4 1H2V9zM5 10v3" stroke-linecap="round" stroke-linejoin="round"/></svg>`,
sliders: `<svg viewBox="0 0 16 16" class="svg-sm" style="color:#f59e0b;" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M2 4h12M2 8h12M2 12h12" stroke-linecap="round"/><circle cx="5" cy="4" r="1.6" fill="currentColor"/><circle cx="11" cy="8" r="1.6" fill="currentColor"/><circle cx="6" cy="12" r="1.6" fill="currentColor"/></svg>`,
checkSmall: `<svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="2" style="width:10px;height:10px;"><path d="M2.5 6.2l2.2 2.2 4.8-5" stroke-linecap="round" stroke-linejoin="round"/></svg>`,
plusSmall: `<svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="2" style="width:10px;height:10px;"><path d="M6 2.5v7M2.5 6h7" stroke-linecap="round"/></svg>`,
checkCircle: `<svg viewBox="0 0 16 16" class="svg-md" style="color:#34d399;" fill="none" stroke="currentColor" stroke-width="2"><circle cx="8" cy="8" r="6"/><path d="M5.2 8.2l2 2 3.8-4" stroke-linecap="round" stroke-linejoin="round"/></svg>`,
alertTriangle: `<svg viewBox="0 0 16 16" class="svg-md" style="color:#f87171;" fill="none" stroke="currentColor" stroke-width="1.9"><path d="M8 2.2L14.3 13H1.7L8 2.2z" stroke-linejoin="round"/><path d="M8 6.3v3.2M8 11.4v.3" stroke-linecap="round"/></svg>`
};
const NATION_FLAGS = {
USA: `<img class="nation-flag-img" src="/ui/branding/flags/usa.png" alt="USA" draggable="false" />`,
GERMANY: `<img class="nation-flag-img" src="/ui/branding/flags/germany.png" alt="Germany" draggable="false" />`,
USSR: `<img class="nation-flag-img" src="/ui/branding/flags/ussr.png" alt="USSR" draggable="false" />`,
UK: `<img class="nation-flag-img" src="/ui/branding/flags/uk.png" alt="UK" draggable="false" />`,
BRITAIN: `<img class="nation-flag-img" src="/ui/branding/flags/uk.png" alt="UK" draggable="false" />`,
JAPAN: `<img class="nation-flag-img" src="/ui/branding/flags/japan.png" alt="Japan" draggable="false" />`,
CHINA: `<img class="nation-flag-img" src="/ui/branding/flags/china.png" alt="China" draggable="false" />`,
ITALY: `<img class="nation-flag-img" src="/ui/branding/flags/italy.png" alt="Italy" draggable="false" />`,
FRANCE: `<img class="nation-flag-img" src="/ui/branding/flags/france.png" alt="France" draggable="false" />`,
SWEDEN: `<img class="nation-flag-img" src="/ui/branding/flags/sweden.png" alt="Sweden" draggable="false" />`,
ISRAEL: `<img class="nation-flag-img" src="/ui/branding/flags/israel.png" alt="Israel" draggable="false" />`
};
const ALIAS_MAP = {
flanker: ["su_27", "su_30", "su_33", "su_34", "j_11", "j_15", "j_16"],
fulcrum: ["mig_29", "mig_35"],
viper: ["f_16"],
fightingfalcon: ["f_16"],
eagle: ["f_15"],
tomcat: ["f_14"],
hornet: ["f_18", "fa_18"],
phantom: ["f-4", "f_4"],
typhoon: ["ef_2000"],
eurofighter: ["ef_2000"],
rafale: ["rafale"],
gripen: ["jas39"],
apache: ["ah_64", "yah_64"],
alligator: ["ka_52"],
blackshark: ["ka_50"],
hind: ["mi_24", "mi_35"],
warthog: ["a_10"],
frogfoot: ["su_25", "su_39"],
abrams: ["abrams", "m1a"],
leopard: ["leopard"],
pantsir: ["pantsyr"],
};
const NATIONS_ORDER = ["USA", "GERMANY", "USSR", "UK", "JAPAN", "CHINA", "ITALY", "FRANCE", "SWEDEN", "ISRAEL"];
const ROMAN_RANKS = ["", "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX"];
const GROUPS_ORDER = ["Without load", "To hit air targets", "To hit ground targets", "Multi-purpose weapon"];
function escHtml(str) {
if (str === null || str === undefined) return "";
return String(str)
.replace(/&/g, "&amp;")
.replace(/</g, "&lt;")
.replace(/>/g, "&gt;")
.replace(/"/g, "&quot;");
}
function renderAircraftSilhouette(category) {
if (category === "HELICOPTERS") {
return `<svg viewBox="0 0 120 60" style="width:100%;height:100%;padding:4px;"><path d="M18,32 L50,30 L72,22 L95,26 L98,33 L70,38 L46,35 L20,38 Z M12,18 L104,18 M62,18 L62,24 M18,32 L14,24" stroke="#64748b" stroke-width="3" fill="#475569" fill-opacity="0.35" stroke-linecap="round"/></svg>`;
}
return `<svg viewBox="0 0 120 60" style="width:100%;height:100%;padding:4px;"><path d="M14,34 L46,29 L66,14 L76,14 L68,28 L98,29 L110,33 L96,36 L68,36 L74,47 L64,47 L48,36 L20,38 Z" stroke="#64748b" stroke-width="2.5" fill="#475569" fill-opacity="0.35" stroke-linejoin="round"/></svg>`;
}
function renderTankSilhouette() {
return `<svg viewBox="0 0 120 60" style="width:100%;height:100%;padding:4px;"><path d="M12,42 C12,38 18,36 30,36 L92,36 C104,36 108,38 108,42 C108,46 104,48 92,48 L30,48 C18,48 12,46 12,42 Z" stroke="#64748b" stroke-width="2" fill="#475569" fill-opacity="0.35"/><rect x="25" y="30" width="68" height="8" rx="2" stroke="#64748b" stroke-width="1.8" fill="#475569" fill-opacity="0.5"/><path d="M45,30 L55,20 L80,20 L84,30 Z" stroke="#64748b" stroke-width="2" fill="#475569" fill-opacity="0.6"/><line x1="80" y1="23" x2="114" y2="23" stroke="#64748b" stroke-width="2.5" stroke-linecap="round"/></svg>`;
}
function renderShipSilhouette() {
return `<svg viewBox="0 0 120 60" style="width:100%;height:100%;padding:4px;"><path d="M10,38 L108,38 L98,48 L22,48 Z M34,38 L38,26 L68,26 L74,38 Z M50,26 L50,16 L54,16 L54,26 Z" stroke="#64748b" stroke-width="2.2" fill="#475569" fill-opacity="0.4" stroke-linejoin="round"/></svg>`;
}
function renderWeaponFallbackSvg(kind, color) {
const c = color || "#64748b";
let shape = `<rect x="5" y="9.5" width="14" height="5" rx="2.5" fill="${c}"/>`;
if (kind && kind.startsWith("aam")) {
shape = `<path d="M5,19 L19,5 M16,5 L19,5 L19,8 M8,16 L5,17 M11,13 L9,11" stroke="${c}" stroke-width="2.3" stroke-linecap="round"/>`;
} else if (kind === "agm" || kind === "atgm") {
shape = `<path d="M4,18 L19,6 M14,6 L19,6 L18,11 M7,15 L10,18" stroke="${c}" stroke-width="2.6" stroke-linecap="round"/>`;
} else if (kind === "gbu" || kind === "bomb") {
shape = `<ellipse cx="12" cy="12" rx="6.5" ry="3.6" transform="rotate(-35 12 12)" fill="${c}"/><path d="M5,17 L8,14" stroke="${c}" stroke-width="2"/>`;
}
return `<svg viewBox="0 0 24 24" style="width:20px;height:20px;display:none;">${shape}</svg>`;
}
function renderWeaponCellHtml(opt) {
if (!opt) return "";
const qtyBadge = opt.count > 1 ? `<span class="wp-cell-qty-badge font-mono-tech">x${opt.count}</span>` : "";
const iconFile = opt.iconFile || opt.icon || "air_to_air_missile.png";
return `
<div class="wp-cell-inner">
${qtyBadge}
<img src="/assets/weapon/${encodeURIComponent(iconFile)}" alt="" class="wp-cell-img" onerror="this.style.display='none'; if(this.nextElementSibling) this.nextElementSibling.style.display='block';" />
${renderWeaponFallbackSvg(opt.kind, opt.color)}
<span class="wp-cell-code" style="color:${escHtml(opt.color)};">${escHtml(opt.code)}</span>
</div>
`;
}
function initImageObserver() {
if ("IntersectionObserver" in window) {
const scrollEl = document.getElementById("tech-tree-scroll");
imageObserver = new IntersectionObserver((entries, obs) => {
entries.forEach(entry => {
if (entry.isIntersecting) {
const img = entry.target;
const src = img.getAttribute("data-src");
if (src) {
img.src = src;
img.removeAttribute("data-src");
}
obs.unobserve(img);
}
});
}, { root: scrollEl || null, rootMargin: "800px 0px" });
}
}