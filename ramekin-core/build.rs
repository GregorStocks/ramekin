fn main() {
    // Generate a unique build ID at compile time.
    // This runs on every build since we don't specify rerun-if-changed.
    let id = uuid::Uuid::new_v4().simple().to_string();
    let short_id: String = id.chars().take(8).collect();
    println!("cargo:rustc-env=BUILD_ID={}", short_id);
}
