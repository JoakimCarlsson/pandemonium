//! Reading a `SHA256SUMS` file and comparing what was downloaded with it.

/// The SHA-256 that `sums`, in the format `sha256sum` writes, lists for the file `name`, in lowercase.
pub(crate) fn expected(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, listed) = line.split_once(char::is_whitespace)?;
        (listed.trim_start().trim_start_matches('*') == name).then(|| hash.to_ascii_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_hash_of_one_file() {
        let sums = "AB12  one.tar.gz\ncd34 *two.zip\n";
        assert_eq!(expected(sums, "one.tar.gz").as_deref(), Some("ab12"));
        assert_eq!(expected(sums, "two.zip").as_deref(), Some("cd34"));
        assert_eq!(expected(sums, "three.zip"), None);
    }
}
