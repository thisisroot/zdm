/// Parses a URL template containing one numbered range, e.g. `part[01-99].zip`,
/// into the concrete list of URLs it expands to. The zero-padding width is
/// taken from how the start number was written — `01` implies two digits.
pub fn parse_batch_pattern(pattern: &str) -> Result<Vec<String>, String> {
    const MAX_BATCH_SIZE: u64 = 500;

    let open = pattern.find('[').ok_or("pattern must contain a range like [01-99]")?;
    let close = pattern[open..].find(']').map(|i| i + open).ok_or("unterminated '[' in pattern")?;
    let inner = &pattern[open + 1..close];
    let (start_str, end_str) = inner.split_once('-').ok_or("range must look like start-end, e.g. 01-99")?;

    let start: u64 = start_str.parse().map_err(|_| format!("'{start_str}' is not a number"))?;
    let end: u64 = end_str.parse().map_err(|_| format!("'{end_str}' is not a number"))?;
    if end < start {
        return Err("range end must not be before its start".to_string());
    }
    if end - start > MAX_BATCH_SIZE {
        return Err(format!("range too large — max {MAX_BATCH_SIZE} files per batch"));
    }

    let width = start_str.len();
    let prefix = &pattern[..open];
    let suffix = &pattern[close + 1..];
    Ok((start..=end).map(|n| format!("{prefix}{n:0width$}{suffix}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_zero_padded_range() {
        let urls = parse_batch_pattern("https://host/part[01-03].zip").unwrap();
        assert_eq!(urls, vec!["https://host/part01.zip", "https://host/part02.zip", "https://host/part03.zip"]);
    }

    #[test]
    fn preserves_prefix_and_suffix_around_the_range() {
        let urls = parse_batch_pattern("https://host/season/ep[1-2]-final.mkv").unwrap();
        assert_eq!(urls, vec!["https://host/season/ep1-final.mkv", "https://host/season/ep2-final.mkv"]);
    }

    #[test]
    fn rejects_missing_range() {
        assert!(parse_batch_pattern("https://host/file.zip").is_err());
    }

    #[test]
    fn rejects_inverted_range() {
        assert!(parse_batch_pattern("https://host/part[09-01].zip").is_err());
    }

    #[test]
    fn rejects_oversized_range() {
        assert!(parse_batch_pattern("https://host/part[0-100000].zip").is_err());
    }
}
