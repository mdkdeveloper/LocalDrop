//! Safe handling of file names received from peers.

/// Turns a peer-supplied relative path into safe path components.
///
/// - splits on `/` and `\`;
/// - drops empty, `.` and `..` components (no directory traversal);
/// - strips characters invalid on Windows/Android/iOS file systems and control chars;
/// - neutralizes Windows reserved names (`CON`, `NUL`, `COM1`, ...);
/// - returns `None` if nothing usable remains.
pub fn sanitize_file_name(name: &str) -> Option<Vec<String>> {
    let parts: Vec<String> = name
        .split(['/', '\\'])
        .filter(|p| !p.is_empty() && *p != "." && *p != "..")
        .filter_map(sanitize_component)
        .collect();
    if parts.is_empty() { None } else { Some(parts) }
}

fn sanitize_component(part: &str) -> Option<String> {
    let cleaned: String = part
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
        .collect();
    let cleaned = cleaned
        .trim_end_matches([' ', '.'])
        .trim_start()
        .to_string();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        return None;
    }
    let stem = cleaned.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit());
    Some(if reserved {
        format!("_{cleaned}")
    } else {
        cleaned
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversal_removed() {
        assert_eq!(
            sanitize_file_name("../../etc/passwd").unwrap(),
            ["etc", "passwd"]
        );
        assert_eq!(sanitize_file_name("..\\..\\win.ini").unwrap(), ["win.ini"]);
        assert_eq!(
            sanitize_file_name("/abs/path.txt").unwrap(),
            ["abs", "path.txt"]
        );
    }

    #[test]
    fn bad_chars_and_reserved() {
        assert_eq!(sanitize_file_name("a:b?.txt").unwrap(), ["ab.txt"]);
        assert_eq!(sanitize_file_name("CON.txt").unwrap(), ["_CON.txt"]);
        assert_eq!(sanitize_file_name("name. ").unwrap(), ["name"]);
    }

    #[test]
    fn empty_rejected() {
        assert!(sanitize_file_name("").is_none());
        assert!(sanitize_file_name("../..").is_none());
        assert!(sanitize_file_name("???").is_none());
    }
}
