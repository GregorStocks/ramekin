fn main() {
    // Generate a unique build ID at compile time
    let id = uuid::Uuid::new_v4().simple().to_string();
    let short_id = &id[..8];
    println!("cargo:rustc-env=BUILD_ID={}", short_id);

    // Re-run if build.rs changes
    println!("cargo:rerun-if-changed=build.rs");
}
