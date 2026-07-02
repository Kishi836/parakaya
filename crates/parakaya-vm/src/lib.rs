#![forbid(unsafe_code)]

pub fn crate_wired() -> bool {
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_compiles() {
        assert!(super::crate_wired());
    }
}
