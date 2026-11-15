use crate::backend::AppState;
use axum::{
    body::Body,
    extract::{Path as AxPath, State},
    http::{header, Response, StatusCode},
    response::IntoResponse,
};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};
use tracing::{debug, info};

const EMBEDDED_LOGO: &[u8] = include_bytes!("../../../ui/branding/logo.png");

static NEGATIVE_IMAGE_CACHE: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();

fn get_negative_cache() -> &'static RwLock<HashSet<String>> {
    NEGATIVE_IMAGE_CACHE.get_or_init(|| RwLock::new(HashSet::new()))
}

static SHARED_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn get_http_client() -> &'static reqwest::Client {
    SHARED_HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(4))
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .pool_max_idle_per_host(25)
            .user_agent("MarmotaVehicleTester/2.0 (Asset Proxy)")
            .build()
            .unwrap_or_default()
    })
}

pub fn get_family_fallback(vid_low: &str) -> Option<&'static str> {
    match vid_low {
        "ah_64a_killstreak" => Some("ah_64a"),
        "h_34_killstreak" => Some("h_34"),
        "mi_4av_killstreak" => Some("mi_4av"),
        "mi_35m_killstreak" => Some("mi_35m"),
        "f_15ex" => Some("f_15e"),
        "germ_leopard_2a8" => Some("germ_leopard_2a7v"),
        "sw_leopard_2a8no" => Some("sw_leopard_2a6nl"),
        "f2a-1_nvidia" => Some("f2a-1"),
        "us_m3a1_stuart_nvidia" => Some("us_m3a1_stuart"),
        "fa_18d_late" => Some("fa_18d_late_malaysia"),
        "f_4e_event" => Some("f-4e"),
        "mig-21_bis_event" => Some("mig-21_bis_lazur"),
        "ju_88g_1_mistel" => Some("ju-88a-4"),
        "uk_olifant_optimering" => Some("uk_olifant_mk_2"),
        "uk_garford_putilov_event" => Some("ussr_garford_putilov"),
        "uk_saint_chamond_event" => Some("fr_saint_chamond"),
        "us_amx_13_75" => Some("fr_amx_13_75"),
        "us_amx_13_90" => Some("fr_amx_13_90"),
        "sw_pv_m39" => Some("sw_pvlvv_fm42"),
        _ => None,
    }
}

pub fn strip_country_suffix(s: &str) -> Option<String> {
    const SUFFIXES: &[&str] = &[
        "_killstreak",
        "_biathlon",
        "_football",
        "_race",
        "_space",
        "_tutorial",
        "_test",
        "_ep",
        "_late",
        "_early",
        "_mod",
        "_default",
        "_usa",
        "_germany",
        "_german",
        "_ussr",
        "_britain",
        "_uk",
        "_japan",
        "_china",
        "_italy",
        "_france",
        "_sweden",
        "_israel",
        "_iaf",
        "_idf",
        "_greece",
        "_spain",
        "_finland",
        "_hungary",
        "_thailand",
        "_india",
        "_belgium",
        "_netherlands",
        "_event",
        "_nvidia",
        "_pre_serie",
        "_2",
        "_1",
        "_0",
    ];
    for sfx in SUFFIXES {
        if let Some(stripped) = s.strip_suffix(sfx) {
            return Some(stripped.to_string());
        }
    }
    None
}

fn png_ok_response(bytes: Vec<u8>) -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .header(header::CACHE_CONTROL, "public, max-age=86400")
        .body(Body::from(bytes))
        .unwrap()
}

struct UnitImageSpec<'a> {
    cache_prefix: &'a str,
    local_dir: &'a str,
    alt_dir: &'a str,
    remote_tex_folder: &'a str,
    extra_dirs: Vec<PathBuf>,
    check_ico_suffix: bool,
    strip_nt_prefix: bool,
}

async fn serve_unit_image(filename: &str, spec: UnitImageSpec<'_>) -> Response<Body> {
    let safe_name = Path::new(filename)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(filename);

    let vid = safe_name.strip_suffix(".png").unwrap_or(safe_name);
    let vid_low = vid.to_lowercase();

    let mut stems = vec![
        vid_low.clone(),
        vid_low.replace('-', "_"),
        vid_low.replace('_', "-"),
        vid.to_string(),
    ];

    if spec.strip_nt_prefix {
        if let Some(stripped_prefix) = vid_low.strip_prefix("nt_") {
            stems.push(stripped_prefix.to_string());
            stems.push(stripped_prefix.replace('_', "-"));
        }
    }

    if let Some(stripped) = strip_country_suffix(&vid_low) {
        stems.push(stripped.clone());
        stems.push(stripped.replace('_', "-"));
        if let Some(inner_strip) = strip_country_suffix(&stripped) {
            stems.push(inner_strip.clone());
            stems.push(inner_strip.replace('_', "-"));
        }
    }
    if let Some(fb) = get_family_fallback(&vid_low) {
        stems.push(fb.to_string());
    }

    let assets_dir = crate::config::get_assets_dir();
    let local_primary = assets_dir.join(spec.local_dir);
    let local_alt = assets_dir.join(spec.alt_dir);

    let mut candidates = Vec::new();
    for s in &stems {
        candidates.push(local_primary.join(format!("{}.png", s)));
        candidates.push(local_alt.join(format!("{}.png", s)));
        for extra in &spec.extra_dirs {
            if extra.exists() {
                candidates.push(extra.join(format!("{}.png", s)));
                if spec.check_ico_suffix {
                    candidates.push(extra.join(format!("{}_ico.png", s)));
                }
            }
        }
    }

    // 1. Non-blocking local disk check
    if let Ok(Some(bytes)) = tokio::task::spawn_blocking(move || {
        for cand in &candidates {
            if cand.exists() {
                if let Ok(b) = fs::read(cand) {
                    return Some(b);
                }
            }
        }
        None
    })
    .await
    {
        return png_ok_response(bytes);
    }

    // 2. Negative cache check
    let neg_key = format!("{}:{}", spec.cache_prefix, vid_low);
    if get_negative_cache()
        .read()
        .map(|g| g.contains(&neg_key))
        .unwrap_or(false)
    {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Image not found"))
            .unwrap();
    }

    // 3. On-demand CDN fetch
    let mut remote_checks = vec![vid_low.clone()];
    for alt in [vid_low.replace('_', "-"), vid_low.replace('-', "_")] {
        if !remote_checks.contains(&alt) {
            remote_checks.push(alt);
        }
    }
    if spec.strip_nt_prefix {
        if let Some(bare) = vid_low.strip_prefix("nt_") {
            for alt in [
                bare.to_string(),
                bare.replace('_', "-"),
                bare.replace('-', "_"),
            ] {
                if !remote_checks.contains(&alt) {
                    remote_checks.push(alt);
                }
            }
        }
    }
    if let Some(stripped) = strip_country_suffix(&vid_low) {
        if !remote_checks.contains(&stripped) {
            remote_checks.push(stripped);
        }
    }
    if let Some(fb) = get_family_fallback(&vid_low) {
        let fb_str = fb.to_string();
        if !remote_checks.contains(&fb_str) {
            remote_checks.push(fb_str);
        }
    }

    let client = get_http_client();
    for s in &remote_checks {
        let remote_url = format!(
            "https://raw.githubusercontent.com/gszabi99/War-Thunder-Datamine/master/tex.vromfs.bin_u/{}/{}.png",
            spec.remote_tex_folder, s
        );
        if let Ok(resp) = client.get(&remote_url).send().await {
            if resp.status().is_success() {
                if let Ok(bytes) = resp.bytes().await {
                    if !bytes.is_empty() {
                        let vec_bytes = bytes.to_vec();
                        let save_dir = local_primary.clone();
                        let dest = save_dir.join(format!("{}.png", vid_low));
                        let copy_bytes = vec_bytes.clone();
                        let _ = tokio::task::spawn_blocking(move || {
                            let _ = fs::create_dir_all(&save_dir);
                            let _ = fs::write(&dest, &copy_bytes);
                        })
                        .await;
                        info!(
                            "[ASSETS.IMAGE_CACHE] Cached {} image: {} ({:.1} KB)",
                            spec.cache_prefix,
                            vid_low,
                            vec_bytes.len() as f64 / 1024.0
                        );
                        return png_ok_response(vec_bytes);
                    }
                }
            }
        }
    }

    if let Ok(mut guard) = get_negative_cache().write() {
        guard.insert(neg_key);
    }
    debug!(
        "[ASSETS.IMAGE_MISS] {} image not found in datamine: {}",
        spec.cache_prefix, vid_low
    );
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Image not found"))
        .unwrap()
}

pub async fn serve_aircraft_image(
    State(state): State<Arc<AppState>>,
    AxPath(filename): AxPath<String>,
) -> impl IntoResponse {
    let cfg = state.config.read().unwrap().clone();
    serve_unit_image(
        &filename,
        UnitImageSpec {
            cache_prefix: "air",
            local_dir: "aircrafts",
            alt_dir: "aircraft",
            remote_tex_folder: "aircrafts",
            extra_dirs: vec![
                PathBuf::from(&cfg.aircrafts_tex_dir),
                PathBuf::from(&cfg.units_icons_dir),
            ],
            check_ico_suffix: true,
            strip_nt_prefix: true,
        },
    )
    .await
}

pub async fn serve_tank_image(
    State(state): State<Arc<AppState>>,
    AxPath(filename): AxPath<String>,
) -> impl IntoResponse {
    let cfg = state.config.read().unwrap().clone();
    serve_unit_image(
        &filename,
        UnitImageSpec {
            cache_prefix: "tank",
            local_dir: "tanks",
            alt_dir: "tank",
            remote_tex_folder: "tanks",
            extra_dirs: vec![PathBuf::from(&cfg.tanks_tex_dir)],
            check_ico_suffix: false,
            strip_nt_prefix: false,
        },
    )
    .await
}

pub async fn serve_ship_image(
    State(state): State<Arc<AppState>>,
    AxPath(filename): AxPath<String>,
) -> impl IntoResponse {
    let cfg = state.config.read().unwrap().clone();
    serve_unit_image(
        &filename,
        UnitImageSpec {
            cache_prefix: "ship",
            local_dir: "ships",
            alt_dir: "ship",
            remote_tex_folder: "ships",
            extra_dirs: vec![PathBuf::from(&cfg.units_icons_dir)],
            check_ico_suffix: true,
            strip_nt_prefix: false,
        },
    )
    .await
}

fn is_he_shell_icon_stem(stem: &str) -> bool {
    let s = stem.trim().to_lowercase();
    if s.is_empty() {
        return false;
    }
    if s.contains("he_frag") || s.starts_with("he_") || s.ends_with("_he") {
        return true;
    }
    s.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|tok| tok == "he" || tok == "hefrag" || tok == "hevt")
}

/// Maps a requested weapon/shell icon filename or stem to safe fallback icon filenames.
/// Air/helicopter ordnance and guided bombs are matched before tank shell rules so substrings
/// like `"heli"` or `"heavy"` never resolve to tank HE shells.
pub fn resolve_weapon_icon_fallbacks(icon_name: &str) -> Vec<&'static str> {
    let stem = Path::new(icon_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(icon_name)
        .to_lowercase();

    let mut out = Vec::new();
    let mut push_unique = |items: &[&'static str]| {
        for &item in items {
            if !out.contains(&item) {
                out.push(item);
            }
        }
    };

    if stem.contains("helfire") || stem.contains("hellfire") {
        push_unique(&[
            "atgm_type_helfire_x2.png",
            "atgm_type_helfire_x4.png",
            "agm_missile.png",
        ]);
    }
    if stem.contains("aim92")
        || stem.contains("stinger")
        || stem.contains("igla")
        || stem.contains("mistral")
    {
        push_unique(&[
            "missile_air_to_air_group_heli.png",
            "air_to_air_missile.png",
        ]);
    }
    if stem.contains("air_to_air") || stem.contains("aam") {
        if stem.contains("midrange") || stem.contains("sarh") || stem.contains("arh") {
            push_unique(&["air_to_air_midrange_missile.png", "air_to_air_missile.png"]);
        } else if stem.contains("heli") {
            push_unique(&[
                "missile_air_to_air_group_heli.png",
                "air_to_air_missile.png",
            ]);
        } else {
            push_unique(&["air_to_air_missile.png", "air_to_air_midrange_missile.png"]);
        }
    }
    if stem.contains("guided_bomb")
        || stem.contains("gbu")
        || stem.contains("egbu")
        || stem.contains("kab")
        || stem.contains("jdam")
        || stem.contains("paveway")
        || (stem.contains("glide") && stem.contains("bomb"))
    {
        push_unique(&[
            "guided_bomb_middle_jdam.png",
            "guided_bomb_red.png",
            "bombs_large.png",
        ]);
    } else if stem.contains("bomb") || stem.contains("mk8") || stem.contains("fab") {
        push_unique(&["bombs_large.png", "bombs_middle.png", "bombs_small.png"]);
    }
    if stem.contains("air_to_earth")
        || stem.contains("air_to_surface")
        || stem.contains("agm")
        || stem.contains("atgm")
        || stem.contains("kh31")
        || stem.contains("kh29")
    {
        push_unique(&["agm_missile.png", "atgm_missile.png"]);
    }
    if stem.contains("nar")
        || stem.contains("hydra")
        || stem.contains("rocket")
        || stem.contains("apkws")
        || stem.contains("sneb")
    {
        push_unique(&["nar_ap_large.png", "atgm_heli_rocket.png"]);
    }
    if stem.contains("m134") || stem.contains("gunpod") {
        push_unique(&["gunpods.png", "m134_new.png"]);
    }
    if stem.contains("fuel") || stem.contains("drop_tank") {
        push_unique(&["fuel_tank_middle.png"]);
    }
    if stem.contains("aiming") || stem.contains("targeting") || stem.contains("pod") {
        push_unique(&["aiming.png"]);
    }
    if stem.contains("missile") {
        push_unique(&["air_to_air_missile.png", "agm_missile.png"]);
    }

    if stem.contains("apfsds") || stem.contains("apds") {
        push_unique(&["apdsfs_tank.png", "apds_tank.png"]);
    }
    if stem.contains("heat") {
        push_unique(&["heat_fs_tank.png", "heat_tank.png"]);
    }
    if stem.contains("smk") || stem.contains("smoke") {
        push_unique(&["smoke_tank.png"]);
    }
    if is_he_shell_icon_stem(&stem) {
        push_unique(&["he_frag_tank.png", "he_frag_base_fuse_tank.png"]);
    }

    out
}

async fn find_first_icon_on_disk(names: Vec<String>, dirs: [PathBuf; 3]) -> Option<Vec<u8>> {
    tokio::task::spawn_blocking(move || {
        for n in &names {
            for dir in &dirs {
                if dir.exists() {
                    let p = dir.join(n);
                    if p.exists() {
                        if let Ok(b) = fs::read(&p) {
                            if !b.is_empty() {
                                return Some(b);
                            }
                        }
                    }
                }
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
}

async fn fetch_and_cache_remote_weapon_icon(
    client: &reqwest::Client,
    local_skin: &Path,
    candidates: &[String],
) -> Option<Vec<u8>> {
    for n in candidates {
        let neg_key = format!("weapon:{}", n);
        if get_negative_cache()
            .read()
            .ok()
            .is_some_and(|c| c.contains(&neg_key))
        {
            continue;
        }
        let remote_url = format!(
            "https://raw.githubusercontent.com/gszabi99/War-Thunder-Datamine/master/atlases.vromfs.bin_u/gameuiskin/{}",
            n
        );
        if let Ok(resp) = client.get(&remote_url).send().await {
            if resp.status().is_success() {
                if let Ok(bytes) = resp.bytes().await {
                    if !bytes.is_empty() {
                        let vec_bytes = bytes.to_vec();
                        let save_dir = local_skin.to_path_buf();
                        let dest = save_dir.join(n);
                        let copy_bytes = vec_bytes.clone();
                        let _ = tokio::task::spawn_blocking(move || {
                            let _ = fs::create_dir_all(&save_dir);
                            let _ = fs::write(&dest, &copy_bytes);
                        })
                        .await;
                        info!(
                            "[ASSETS.WEAPON_CACHE] Cached weapon icon: {} ({:.1} KB)",
                            n,
                            vec_bytes.len() as f64 / 1024.0
                        );
                        return Some(vec_bytes);
                    }
                }
            } else if resp.status() == reqwest::StatusCode::NOT_FOUND {
                if let Ok(mut neg) = get_negative_cache().write() {
                    neg.insert(neg_key);
                }
            }
        }
    }
    None
}

pub async fn serve_weapon_icon(
    State(state): State<Arc<AppState>>,
    AxPath(filename): AxPath<String>,
) -> impl IntoResponse {
    let safe_name = Path::new(&filename)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&filename)
        .to_string();

    let stem = safe_name
        .strip_suffix(".png")
        .unwrap_or(&safe_name)
        .to_lowercase();
    let cfg = state.config.read().unwrap().clone();
    let local_skin = crate::config::get_assets_dir().join("gameuiskin");
    let alt_skin = crate::config::get_assets_dir().join("weapon");
    let skin_dir = PathBuf::from(&cfg.gameuiskin_dir);

    let mut exact_names = vec![safe_name.clone()];
    let lower_name = format!("{}.png", stem);
    if lower_name != safe_name {
        exact_names.push(lower_name);
    }

    // 1. Try exact icon file on disk before any fallback substitution.
    if let Some(bytes) = find_first_icon_on_disk(
        exact_names.clone(),
        [local_skin.clone(), alt_skin.clone(), skin_dir.clone()],
    )
    .await
    {
        return png_ok_response(bytes);
    }

    // 2. Try downloading the exact icon file from Datamine CDN.
    let client = get_http_client();
    if let Some(bytes) = fetch_and_cache_remote_weapon_icon(client, &local_skin, &exact_names).await
    {
        return png_ok_response(bytes);
    }

    // 3. Only if exact icon failed both locally and remotely, resolve non-colliding fallbacks.
    let fallback_names: Vec<String> = resolve_weapon_icon_fallbacks(&stem)
        .into_iter()
        .map(String::from)
        .collect();
    if !fallback_names.is_empty() {
        if let Some(bytes) = find_first_icon_on_disk(
            fallback_names.clone(),
            [local_skin.clone(), alt_skin.clone(), skin_dir.clone()],
        )
        .await
        {
            return png_ok_response(bytes);
        }
        if let Some(bytes) =
            fetch_and_cache_remote_weapon_icon(client, &local_skin, &fallback_names).await
        {
            return png_ok_response(bytes);
        }
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Weapon icon not found"))
        .unwrap()
}

pub async fn serve_branding_image(AxPath(filename): AxPath<String>) -> impl IntoResponse {
    let safe_name = Path::new(&filename)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&filename);

    let assets_dir = crate::config::get_assets_dir();
    let file_path = assets_dir.join("branding").join(safe_name);
    if let Ok(bytes) = tokio::fs::read(&file_path).await {
        let mime = mime_guess::from_path(safe_name).first_or_octet_stream();
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CACHE_CONTROL, "public, max-age=86400")
            .body(Body::from(bytes))
            .unwrap();
    }

    if safe_name == "logo.png" {
        return png_ok_response(EMBEDDED_LOGO.to_vec());
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Branding asset not found"))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::{is_he_shell_icon_stem, resolve_weapon_icon_fallbacks};

    #[test]
    fn test_air_ordnance_never_falls_back_to_tank_he_shell() {
        for icon in [
            "missile_air_to_air_group_heli.png",
            "guided_bomb_heavy.png",
            "guided_bomb_heavy_x2.png",
            "guided_bomb_laser_heavy.png",
            "egbu_24_heavy.png",
            "bombs_heavy_middle.png",
            "atgm_type_helfire_x2.png",
            "guided_bomb_thermobaric.png",
        ] {
            let fallbacks = resolve_weapon_icon_fallbacks(icon);
            assert!(
                !fallbacks.contains(&"he_frag_tank.png"),
                "Air/heli weapon icon '{}' unexpectedly fell back to he_frag_tank.png: {:?}",
                icon,
                fallbacks
            );
        }
    }

    #[test]
    fn test_actual_he_tank_shells_still_resolve_to_he_icon() {
        for icon in [
            "he_tank.png",
            "he_frag_radio_fuse.png",
            "152mm_he.png",
            "sap_he_naval.png",
        ] {
            assert!(
                is_he_shell_icon_stem(icon.trim_end_matches(".png")),
                "Expected HE match for {}",
                icon
            );
            let fallbacks = resolve_weapon_icon_fallbacks(icon);
            assert!(
                fallbacks.contains(&"he_frag_tank.png"),
                "Tank/naval HE shell '{}' should fall back to he_frag_tank.png: {:?}",
                icon,
                fallbacks
            );
        }
    }
}
