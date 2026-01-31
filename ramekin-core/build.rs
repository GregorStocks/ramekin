fn main() {
    // Generate a unique build ID at compile time.
    // This runs on every build since we don't specify rerun-if-changed.
    let id = uuid::Uuid::new_v4().simple().to_string();
    // Safe: UUID simple format is hex (ASCII only)
    #[allow(clippy::string_slice)]
    let short_id = &id[..8];
    println!("cargo:rustc-env=BUILD_ID={}", short_id);
}
