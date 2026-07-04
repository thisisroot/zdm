/// Turns a URL into a filesystem-safe filename. Windows has the strictest
/// rules of the three target platforms, so sanitizing for it also satisfies
/// macOS and Linux — a name that's safe on Windows never fails there.
pub fn filename_from_url(url: &str) -> String {
    let last_segment = url.rsplit('/').find(|s| !s.is_empty()).unwrap_or("download");

    // Query strings and fragments ride along in the last path segment (e.g.
    // `file.rar?726013813`) but were never part of the filename — left in,
    // `?` alone is enough to make Windows refuse to create the file at all
    // (OS error 123, "the filename... syntax is incorrect").
    let without_query = last_segment.split(['?', '#']).next().unwrap_or(last_segment);

    let decoded = percent_decode(without_query);
    sanitize(&decoded)
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    out.push(byte);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    // Falls back to the original (still-encoded) text if the decoded bytes
    // aren't valid UTF-8, rather than losing the filename entirely.
    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

const RESERVED_WINDOWS_NAMES: &[&str] =
    &["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"];

const MAX_NAME_LEN: usize = 180;

fn sanitize(input: &str) -> String {
    let replaced: String =
        input.chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c }).collect();

    // Collapse runs of underscores introduced by replacing multiple invalid
    // characters in a row (e.g. `<>:` → `___`) into one, purely cosmetic.
    let mut out = String::with_capacity(replaced.len());
    let mut last_was_underscore = false;
    for c in replaced.chars() {
        if c == '_' {
            if !last_was_underscore {
                out.push(c);
            }
            last_was_underscore = true;
        } else {
            out.push(c);
            last_was_underscore = false;
        }
    }

    // Windows also rejects names ending in a dot or space.
    while out.ends_with('.') || out.ends_with(' ') {
        out.pop();
    }

    if out.is_empty() {
        return "download".to_string();
    }

    let stem = out.split('.').next().unwrap_or(&out);
    if RESERVED_WINDOWS_NAMES.iter().any(|reserved| reserved.eq_ignore_ascii_case(stem)) {
        out = format!("_{out}");
    }

    if out.len() > MAX_NAME_LEN {
        out.truncate(MAX_NAME_LEN);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_query_string_that_would_break_windows() {
        let url = "https://d121.pgupgame.com/d112/game/pc/The.Last.of.Us.Part.I/The.Last.of.Us.Part.I.Deluxe.Edition.v1.1.2.0-ElAmigos-Par30Game.part01.rar?726013813";
        assert_eq!(filename_from_url(url), "The.Last.of.Us.Part.I.Deluxe.Edition.v1.1.2.0-ElAmigos-Par30Game.part01.rar");
    }

    #[test]
    fn strips_fragment() {
        assert_eq!(filename_from_url("https://host/file.zip#section"), "file.zip");
    }

    #[test]
    fn decodes_percent_encoding() {
        assert_eq!(filename_from_url("https://host/My%20File%20Name.pdf"), "My File Name.pdf");
    }

    #[test]
    fn replaces_forbidden_characters() {
        assert_eq!(filename_from_url("https://host/weird<name>:file.txt"), "weird_name_file.txt");
    }

    #[test]
    fn trims_trailing_dots_and_spaces() {
        assert_eq!(filename_from_url("https://host/file.txt%2E%20"), "file.txt");
    }

    #[test]
    fn falls_back_when_no_path_segment_exists_at_all() {
        assert_eq!(filename_from_url(""), "download");
        assert_eq!(filename_from_url("///"), "download");
    }

    #[test]
    fn escapes_reserved_windows_device_names() {
        assert_eq!(filename_from_url("https://host/CON"), "_CON");
        assert_eq!(filename_from_url("https://host/con.txt"), "_con.txt");
    }

    #[test]
    fn caps_extremely_long_names() {
        let long = "a".repeat(500);
        let result = filename_from_url(&format!("https://host/{long}.zip"));
        assert!(result.len() <= MAX_NAME_LEN);
    }
}
