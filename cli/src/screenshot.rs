use anyhow::{Context, Result};
use headless_chrome::protocol::cdp::Emulation::SetDeviceMetricsOverride;
use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use headless_chrome::Browser;
use std::path::{Path, PathBuf};
use std::sync::Arc;

// Mobile viewport: iPhone-like width, but tall to see content below the fold
const MOBILE_WIDTH: u32 = 375;
const MOBILE_HEIGHT: u32 = 1200;
const MOBILE_DEVICE_SCALE_FACTOR: f64 = 2.0;

type Tab = Arc<headless_chrome::Tab>;

/// Find Chrome/Chromium executable, checking Playwright cache first
fn find_chrome() -> Option<PathBuf> {
    // Check CHROME environment variable first
    if let Ok(chrome_path) = std::env::var("CHROME") {
        let path = PathBuf::from(&chrome_path);
        if path.exists() {
            tracing::debug!(path = %path.display(), "Using Chrome from CHROME env var");
            return Some(path);
        }
    }

    // Check Playwright cache directories (sorted by version, newest first)
    if let Ok(home) = std::env::var("HOME") {
        let playwright_cache = PathBuf::from(&home).join(".cache/ms-playwright");
        if playwright_cache.exists() {
            if let Ok(entries) = std::fs::read_dir(&playwright_cache) {
                let mut chrome_dirs: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_name().to_string_lossy().starts_with("chromium-"))
                    .collect();
                // Sort by name descending to get newest version first
                chrome_dirs.sort_by_key(|b| std::cmp::Reverse(b.file_name()));

                for dir in chrome_dirs {
                    // Try common Chrome binary locations within Playwright dirs
                    for subpath in &["chrome-linux64/chrome", "chrome-linux/chrome"] {
                        let chrome_path = dir.path().join(subpath);
                        if chrome_path.exists() {
                            tracing::debug!(path = %chrome_path.display(), "Found Chrome in Playwright cache");
                            return Some(chrome_path);
                        }
                    }
                }
            }
        }
    }

    // Let headless_chrome try its default detection
    tracing::debug!("No Chrome found in Playwright cache, using default detection");
    None
}

/// Set device metrics for proper viewport emulation
/// For mobile: sets mobile=true and device_scale_factor=2.0 for retina display
fn set_device_metrics(tab: &Tab, width: u32, height: u32, mobile: bool) -> Result<()> {
    let device_scale_factor = if mobile {
        MOBILE_DEVICE_SCALE_FACTOR
    } else {
        1.0
    };

    tab.call_method(SetDeviceMetricsOverride {
        width,
        height,
        device_scale_factor,
        mobile,
        scale: None,
        screen_width: None,
        screen_height: None,
        position_x: None,
        position_y: None,
        dont_set_visible_size: None,
        screen_orientation: None,
        viewport: None,
        display_feature: None,
        device_posture: None,
    })
    .context("Failed to set device metrics")?;
    Ok(())
}

/// Capture a screenshot and save it to a file
fn capture_and_save(tab: &Tab, path: &Path) -> Result<()> {
    let png = tab
        .capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, true)
        .context("Failed to capture screenshot")?;
    std::fs::write(path, &png).context("Failed to write screenshot")?;
    tracing::debug!(path = %path.display(), "Saved screenshot");
    Ok(())
}

/// Take screenshots of the app as the test user
pub fn screenshot(
    ui_url: &str,
    username: &str,
    password: &str,
    output_dir: &Path,
    width: u32,
    height: u32,
) -> Result<()> {
    tracing::debug!("Starting screenshot capture");
    tracing::debug!(url = %ui_url, "UI URL");
    tracing::debug!(path = %output_dir.display(), "Output directory");
    tracing::debug!(width, height, "Desktop viewport size");
    tracing::debug!(MOBILE_WIDTH, MOBILE_HEIGHT, "Mobile viewport size");

    std::fs::create_dir_all(output_dir).context("Failed to create output directory")?;
    tracing::debug!("Output directory created/verified");

    // Launch browser with no-sandbox for Linux compatibility
    tracing::debug!("Launching headless Chrome...");
    let chrome_path = find_chrome();
    if let Some(ref path) = chrome_path {
        tracing::info!(path = %path.display(), "Using Chrome");
    }

    let mut builder = headless_chrome::LaunchOptions::default_builder();
    builder
        .args(vec![
            std::ffi::OsStr::new("--no-sandbox"),
            std::ffi::OsStr::new("--disable-dev-shm-usage"),
            std::ffi::OsStr::new("--ignore-certificate-errors"),
        ])
        .path(chrome_path);

    let browser = Browser::new(builder.build().expect("Failed to build launch options"))
        .context("Failed to launch browser")?;
    tracing::debug!("Browser launched successfully");

    let tab = browser.new_tab().context("Failed to create tab")?;
    let tab = Arc::new(tab);
    tracing::debug!("New tab created");

    // Set desktop viewport size (mobile=false)
    set_device_metrics(&tab, width, height, false)?;
    tracing::debug!(width, height, "Desktop viewport set");

    // Navigate to login page
    tracing::debug!(url = %ui_url, "Navigating to login page...");
    tab.navigate_to(ui_url)
        .context("Failed to navigate to UI")?;
    tracing::debug!("Navigation complete, waiting for login form...");
    tab.wait_for_element("input[type='text']")
        .context("Failed to find username input")?;
    tracing::debug!("Login form found");

    // Log in
    tracing::debug!("Entering credentials...");
    tab.wait_for_element("input[type='text']")
        .context("Failed to find username input")?
        .click()
        .context("Failed to click username input")?
        .type_into(username)
        .context("Failed to type username")?;
    tab.wait_for_element("input[type='password']")
        .context("Failed to find password input")?
        .click()
        .context("Failed to click password input")?
        .type_into(password)
        .context("Failed to type password")?;
    tracing::debug!("Submitting login form...");
    tab.wait_for_element("button[type='submit']")
        .context("Failed to find submit button")?
        .click()
        .context("Failed to click submit")?;

    // Wait for cookbook page to load
    tracing::debug!("Waiting for cookbook page (looking for .recipe-card)...");
    tab.wait_for_element(".recipe-card")
        .context("Failed to wait for recipe cards")?;
    tracing::debug!("Cookbook page loaded");

    // Store the cookbook URL for later
    let cookbook_url = tab.get_url();

    // ===== DESKTOP SCREENSHOTS =====
    tracing::info!("Taking desktop screenshots...");

    // Screenshot 1: Cookbook page
    tracing::debug!("Capturing cookbook screenshot...");
    capture_and_save(&tab, &output_dir.join("cookbook.png"))?;

    // Click on first recipe card
    tracing::debug!("Clicking on first recipe card...");
    tab.wait_for_element(".recipe-card")
        .context("Failed to find recipe card")?
        .click()
        .context("Failed to click recipe card")?;
    // Wait for recipe page to load (instructions section is always present)
    tracing::debug!("Waiting for recipe page (looking for .instructions)...");
    tab.wait_for_element(".instructions")
        .context("Failed to wait for recipe page")?;
    // Give photos time to load if they exist
    tracing::debug!("Recipe page loaded, waiting for images...");
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Store the recipe URL for mobile screenshots
    let recipe_url = tab.get_url();

    // Screenshot 2: Recipe page
    tracing::debug!("Capturing recipe screenshot...");
    capture_and_save(&tab, &output_dir.join("recipe.png"))?;

    // Screenshot 3: Edit page
    let edit_url = format!("{}/edit", recipe_url);
    tracing::debug!(url = %edit_url, "Navigating to edit page...");
    tab.navigate_to(&edit_url)
        .context("Failed to navigate to edit page")?;
    tracing::debug!("Waiting for edit form (looking for textarea)...");
    tab.wait_for_element("textarea")
        .context("Failed to wait for edit form")?;

    tracing::debug!("Capturing edit screenshot...");
    capture_and_save(&tab, &output_dir.join("edit.png"))?;

    // ===== MOBILE SCREENSHOTS =====
    tracing::info!("Taking mobile screenshots...");

    // Switch to mobile viewport with mobile=true and device_scale_factor=2.0
    set_device_metrics(&tab, MOBILE_WIDTH, MOBILE_HEIGHT, true)?;
    tracing::debug!(
        MOBILE_WIDTH,
        MOBILE_HEIGHT,
        "Mobile viewport set (mobile=true, scale=2x)"
    );

    // Navigate back to cookbook page
    tracing::debug!(url = %cookbook_url, "Navigating to cookbook page for mobile...");
    tab.navigate_to(&cookbook_url)
        .context("Failed to navigate to cookbook")?;
    tab.wait_for_element(".recipe-card")
        .context("Failed to wait for recipe cards")?;
    std::thread::sleep(std::time::Duration::from_millis(300));

    // Mobile screenshot 1: Cookbook page
    tracing::debug!("Capturing mobile cookbook screenshot...");
    capture_and_save(&tab, &output_dir.join("cookbook-mobile.png"))?;

    // Navigate to recipe page
    tracing::debug!(url = %recipe_url, "Navigating to recipe page for mobile...");
    tab.navigate_to(&recipe_url)
        .context("Failed to navigate to recipe")?;
    tab.wait_for_element(".instructions")
        .context("Failed to wait for recipe page")?;
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Mobile screenshot 2: Recipe page
    tracing::debug!("Capturing mobile recipe screenshot...");
    capture_and_save(&tab, &output_dir.join("recipe-mobile.png"))?;

    // Navigate to edit page
    tracing::debug!(url = %edit_url, "Navigating to edit page for mobile...");
    tab.navigate_to(&edit_url)
        .context("Failed to navigate to edit page")?;
    tab.wait_for_element("textarea")
        .context("Failed to wait for edit form")?;

    // Mobile screenshot 3: Edit page
    tracing::debug!("Capturing mobile edit screenshot...");
    capture_and_save(&tab, &output_dir.join("edit-mobile.png"))?;

    tracing::info!("All screenshots captured successfully (desktop and mobile)");
    Ok(())
}
