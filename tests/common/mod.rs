pub fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/sigma/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {path}: {e}"))
}
