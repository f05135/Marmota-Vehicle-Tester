//! Comment- and string-aware lexer and surgical editor for Dagor Engine `.blk` files.

use regex::Regex;

/// Scans `src` starting at `open_brace_idx` (which must point to `{`) and returns the
/// byte offset immediately after the matching closing `}`, skipping `//`, `/* */`, and `"..."`.
pub fn find_matching_brace(src: &str, open_brace_idx: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    if bytes.get(open_brace_idx).copied() != Some(b'{') {
        return None;
    }

    let mut i = open_brace_idx;
    let mut depth: i32 = 0;

    while i < bytes.len() {
        let b = bytes[i];
        // Skip string literals
        if b == b'"' {
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        // Skip line comments //
        if b == b'/' && bytes.get(i + 1).copied() == Some(b'/') {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        // Skip block comments /* ... */
        if b == b'/' && bytes.get(i + 1).copied() == Some(b'*') {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            continue;
        }

        if b == b'{' {
            depth += 1;
        } else if b == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some(i + 1);
            }
        }
        i += 1;
    }

    None
}

/// Returns `(block_start, open_brace_idx, block_end)` for the first top-level or nested block
/// whose identifier equals `block_name` inside `src`.
pub fn find_named_block(src: &str, block_name: &str) -> Option<(usize, usize, usize)> {
    let pattern = format!(r"(?m)\b{}\s*\{{", regex::escape(block_name));
    let re = Regex::new(&pattern).ok()?;
    for m in re.find_iter(src) {
        if is_inside_comment_or_string(src, m.start()) {
            continue;
        }
        let open_rel = src[m.start()..m.end()].rfind('{')?;
        let open_brace = m.start() + open_rel;
        if let Some(end) = find_matching_brace(src, open_brace) {
            return Some((m.start(), open_brace, end));
        }
    }
    None
}

/// Checks whether byte offset `pos` falls inside a string literal or comment.
pub fn is_inside_comment_or_string(src: &str, pos: usize) -> bool {
    let bytes = src.as_bytes();
    let mut i = 0usize;
    let limit = pos.min(bytes.len());
    while i < limit {
        let b = bytes[i];
        if b == b'"' {
            i += 1;
            while i < bytes.len() {
                if i >= limit {
                    return true;
                }
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if b == b'/' && bytes.get(i + 1).copied() == Some(b'/') {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                if i >= limit {
                    return true;
                }
                i += 1;
            }
            if i >= limit {
                return true;
            }
            continue;
        }
        if b == b'/' && bytes.get(i + 1).copied() == Some(b'*') {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                if i >= limit {
                    return true;
                }
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            if i >= limit {
                return true;
            }
            continue;
        }
        i += 1;
    }
    false
}

/// Locates the player unit block inside `units { ... }` matching `name:t="<player_wing>"`.
/// Returns `(start_of_unit_type_ident, end_after_closing_brace, unit_type_ident)`.
pub fn find_unit_block_by_name(
    src: &str,
    units_open_brace: usize,
    player_wing: &str,
) -> Option<(usize, usize, String)> {
    let units_end = find_matching_brace(src, units_open_brace)?;
    let units_slice = &src[units_open_brace..units_end];
    let name_pat = format!(r#"\bname(?::t)?\s*=\s*"{}""#, regex::escape(player_wing));
    let name_re = Regex::new(&name_pat).ok()?;

    for m in name_re.find_iter(units_slice) {
        let abs_name = units_open_brace + m.start();
        if is_inside_comment_or_string(src, abs_name) {
            continue;
        }
        // Walk backwards inside units_slice to find the opening brace at depth 1 (immediate child of units)
        let mut depth = 0i32;
        let bytes = src.as_bytes();
        let mut idx = abs_name;
        let mut unit_open = None;
        while idx > units_open_brace + 1 {
            idx -= 1;
            if bytes[idx] == b'}' && !is_inside_comment_or_string(src, idx) {
                depth += 1;
            } else if bytes[idx] == b'{' && !is_inside_comment_or_string(src, idx) {
                if depth == 0 {
                    unit_open = Some(idx);
                    break;
                }
                depth -= 1;
            }
        }
        if let Some(open_brace) = unit_open {
            let before_brace = src[..open_brace].trim_end();
            let type_start = before_brace
                .rfind(|c: char| c.is_whitespace() || c == '{' || c == '}')
                .map(|p| p + 1)
                .unwrap_or(0);
            let unit_type = before_brace[type_start..].trim().to_string();
            if let Some(end_pos) = find_matching_brace(src, open_brace) {
                return Some((type_start, end_pos, unit_type));
            }
        }
    }
    None
}

/// Removes all sub-blocks with the given name (e.g. `plane { ... }`) safely with brace balancing.
pub fn strip_named_subblocks(block: &str, subblock_name: &str) -> String {
    let mut out = block.to_string();
    let pat = format!(r"(?m)\b{}\s*\{{", regex::escape(subblock_name));
    let Ok(re) = Regex::new(&pat) else {
        return out;
    };
    loop {
        let mut removed = false;
        let snapshot = out.clone();
        for m in re.find_iter(&snapshot) {
            if is_inside_comment_or_string(&snapshot, m.start()) {
                continue;
            }
            if let Some(open_rel) = snapshot[m.start()..m.end()].rfind('{') {
                let open_idx = m.start() + open_rel;
                if let Some(end_idx) = find_matching_brace(&snapshot, open_idx) {
                    // Also eat leading horizontal whitespace on the same line
                    let line_start = snapshot[..m.start()]
                        .rfind('\n')
                        .map(|p| p + 1)
                        .unwrap_or(0);
                    let start_trim = if snapshot[line_start..m.start()].trim().is_empty() {
                        line_start
                    } else {
                        m.start()
                    };
                    out.replace_range(start_trim..end_idx, "");
                    removed = true;
                    break;
                }
            }
        }
        if !removed {
            break;
        }
    }
    out
}

/// Extracts `tm:m=[[...]]` y-coordinate (altitude) from a unit block if present.
pub fn extract_tm_altitude(block: &str) -> Option<f64> {
    let re = Regex::new(
        r#"\btm(?::m)?\s*=\s*\[\s*\[[^\]]*\]\s*\[[^\]]*\]\s*\[[^\]]*\]\s*\[\s*([-+]?\d*\.?\d+)\s*,\s*([-+]?\d*\.?\d+)\s*,\s*([-+]?\d*\.?\d+)\s*\]\s*\]"#,
    )
    .ok()?;
    re.captures(block)
        .and_then(|c| c.get(2))
        .and_then(|m| m.as_str().parse::<f64>().ok())
}

/// Replaces the y-coordinate in `tm:m=[[...][...][...][x, y, z]]`.
pub fn set_tm_altitude(block: &str, new_y: f64) -> String {
    let Ok(re) = Regex::new(
        r#"(\btm(?::m)?\s*=\s*\[\s*\[[^\]]*\]\s*\[[^\]]*\]\s*\[[^\]]*\]\s*\[\s*[-+]?\d*\.?\d+\s*,\s*)([-+]?\d*\.?\d+)(\s*,\s*[-+]?\d*\.?\d+\s*\]\s*\])"#,
    ) else {
        return block.to_string();
    };
    re.replace(block, |caps: &regex::Captures| {
        format!("{}{:.2}{}", &caps[1], new_y, &caps[3])
    })
    .to_string()
}

/// Extracts a quoted string field inside a block (ignoring comments).
pub fn extract_string_field(block: &str, key: &str) -> Option<String> {
    let pat = format!(r#"\b{}(?::t)?\s*=\s*"([^"]*)""#, regex::escape(key));
    let re = Regex::new(&pat).ok()?;
    for caps in re.captures_iter(block) {
        if let Some(m) = caps.get(0) {
            if !is_inside_comment_or_string(block, m.start()) {
                return caps.get(1).map(|v| v.as_str().to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brace_matching_ignores_strings_and_comments() {
        let src = r#"
        armada {
            name:t = "wing { fake } //"
            // comment with { open brace
            /* block comment } { */
            tm:m = [[1.0, 0.0, 0.0] [0.0, 1.0, 0.0] [0.0, 0.0, 1.0] [10.0, 525.5, 20.0]]
            plane {
                wing_name:t = "a"
            }
        }
        "#;
        let open = src.find('{').unwrap();
        let close = find_matching_brace(src, open).unwrap();
        assert_eq!(src[..close].trim_end().chars().last(), Some('}'));
        assert!(src[open..close].contains("plane"));

        let stripped = strip_named_subblocks(&src[open - 7..close], "plane");
        assert!(!stripped.contains("wing_name"));
        assert!(stripped.contains("tm:m"));
    }

    #[test]
    fn test_find_unit_and_altitude_roundtrip() {
        let mission = r#"
        mission_settings { name:t = "test" }
        units {
            armada {
                name:t = "player_01"
                tm:m = [[1, 0, 0] [0, 1, 0] [0, 0, 1] [100.0, 1200.0, 200.0]]
            }
        }
        "#;
        let (_, units_open, _) =
            find_named_block(mission, "units").expect("units block must exist");
        let (s, e, utype) =
            find_unit_block_by_name(mission, units_open, "player_01").expect("unit must be found");
        assert_eq!(utype, "armada");
        let block = &mission[s..e];
        assert_eq!(extract_tm_altitude(block), Some(1200.0));
        let updated = set_tm_altitude(block, 3500.25);
        assert_eq!(extract_tm_altitude(&updated), Some(3500.25));
    }
}
