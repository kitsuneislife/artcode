use jit::ArtCache;

#[test]
fn compute_hash_is_deterministic() {
    let a = ArtCache::compute_hash("hello world");
    let b = ArtCache::compute_hash("hello world");
    assert_eq!(a, b, "hashes must be stable across calls");
}

#[test]
fn different_inputs_produce_different_hashes() {
    let a = ArtCache::compute_hash("func @foo() -> i64 { ret %v0 }");
    let b = ArtCache::compute_hash("func @bar() -> i64 { ret %v1 }");
    assert_ne!(a, b, "distinct IR must yield distinct hashes");
}

#[test]
fn cache_roundtrip() {
    let cache = ArtCache::new();
    let hash = ArtCache::compute_hash("ir_payload_test_roundtrip");
    // Make prefix unique across parallel test invocations
    let prefix = format!("test_{}", std::process::id());

    // Initially no entry
    assert!(
        cache.get(&prefix, &hash, "ll").is_none(),
        "cache should be cold before set"
    );

    // Write an entry
    let content = "define i64 @foo() { ... }";
    cache.set(&prefix, &hash, "ll", content);

    // Read back
    let got = cache.get(&prefix, &hash, "ll").expect("cache should return entry after set");
    assert_eq!(got, content, "roundtrip value must match");
}

#[test]
fn cache_different_extensions_are_independent() {
    let cache = ArtCache::new();
    let hash = ArtCache::compute_hash("ext_independence_test");
    let prefix = format!("exttest_{}", std::process::id());

    cache.set(&prefix, &hash, "ll", "llvm-ir-content");
    cache.set(&prefix, &hash, "c", "c-code-content");

    let ll = cache.get(&prefix, &hash, "ll").unwrap();
    let c = cache.get(&prefix, &hash, "c").unwrap();

    assert_eq!(ll, "llvm-ir-content");
    assert_eq!(c, "c-code-content");
    assert_ne!(ll, c);
}
