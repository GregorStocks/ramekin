use anyhow::{Context, Result};
use headless_chrome::Browser;
use std::path::Path;

/// Take screenshots of the app as the test user
pub fn screenshot(
    ui_url: &str,
    username: &str,
    password: &str,
    output_dir: &Path,
    width: u32,
    height: u32,
) -> Result<()> {
    eprintln!("[screenshot] Starting screenshot capture");
    eprintln!("[screenshot] UI URL: {}", ui_url);
    eprintln!("[screenshot] Output dir: {}", output_dir.display());
    eprintln!("[screenshot] Viewport: {}x{}", width, height);

    std::fs::create_dir_all(output_dir).context("Failed to create output directory")?;
    eprintln!("[screenshot] Output directory created/verified");

    // Launch browser with no-sandbox for Linux compatibility
    eprintln!("[screenshot] Launching headless Chrome...");
    let browser = Browser::new(
        headless_chrome::LaunchOptions::default_builder()
            .args(vec![
                std::ffi::OsStr::new("--no-sandbox"),
                std::ffi::OsStr::new("--disable-dev-shm-usage"),
            ])
            .build()
            .expect("Failed to build launch options"),
    )
    .context("Failed to launch browser")?;
    eprintln!("[screenshot] Browser launched successfully");

    let tab = browser.new_tab().context("Failed to create tab")?;
    eprintln!("[screenshot] New tab created");

    // Set viewport size
    tab.set_bounds(headless_chrome::types::Bounds::Normal {
        left: Some(0),
        top: Some(0),
        width: Some(width as f64),
        height: Some(height as f64),
    })
    .context("Failed to set viewport")?;
    eprintln!("[screenshot] Viewport set to {}x{}", width, height);

    // Navigate to login page
    eprintln!("[screenshot] Navigating to {}...", ui_url);
    tab.navigate_to(ui_url)
        .context("Failed to navigate to UI")?;
    eprintln!("[screenshot] Navigation complete, waiting for login form...");
    tab.wait_for_element("input[type='text']")
        .context("Failed to find username input")?;
    eprintln!("[screenshot] Login form found");

    // Log in
    eprintln!("[screenshot] Entering credentials...");
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
    eprintln!("[screenshot] Submitting login form...");
    tab.wait_for_element("button[type='submit']")
        .context("Failed to find submit button")?
        .click()
        .context("Failed to click submit")?;

    // Wait for cookbook page to load
    eprintln!("[screenshot] Waiting for cookbook page (looking for .recipe-card)...");
    tab.wait_for_element(".recipe-card")
        .context("Failed to wait for recipe cards")?;
    eprintln!("[screenshot] Cookbook page loaded");

    // Screenshot 1: Cookbook page
    eprintln!("[screenshot] Capturing cookbook screenshot...");
    let cookbook_path = output_dir.join("cookbook.png");
    let cookbook_png = tab
        .capture_screenshot(
            headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png,
            None,
            None,
            true,
        )
        .context("Failed to capture cookbook screenshot")?;
    std::fs::write(&cookbook_path, &cookbook_png).context("Failed to write cookbook screenshot")?;
    eprintln!("[screenshot] Saved: {}", cookbook_path.display());

    // Click on first recipe card
    eprintln!("[screenshot] Clicking on first recipe card...");
    tab.wait_for_element(".recipe-card")
        .context("Failed to find recipe card")?
        .click()
        .context("Failed to click recipe card")?;
    // Wait for recipe page to load (instructions section is always present)
    eprintln!("[screenshot] Waiting for recipe page (looking for .instructions)...");
    tab.wait_for_element(".instructions")
        .context("Failed to wait for recipe page")?;
    // Give photos time to load if they exist
    eprintln!("[screenshot] Recipe page loaded, waiting for images...");
    std::thread::sleep(std::time::Duration::from_millis(500));

    eprintln!("[screenshot] Capturing recipe screenshot...");
    let recipe_path = output_dir.join("recipe.png");
    let recipe_png = tab
        .capture_screenshot(
            headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png,
            None,
            None,
            true,
        )
        .context("Failed to capture recipe screenshot")?;
    std::fs::write(&recipe_path, &recipe_png).context("Failed to write recipe screenshot")?;
    eprintln!("[screenshot] Saved: {}", recipe_path.display());

    // Screenshot 3: Edit page
    let recipe_url = tab.get_url();
    let edit_url = format!("{}/edit", recipe_url);
    eprintln!("[screenshot] Navigating to edit page: {}...", edit_url);
    tab.navigate_to(&edit_url)
        .context("Failed to navigate to edit page")?;
    eprintln!("[screenshot] Waiting for edit form (looking for textarea)...");
    tab.wait_for_element("textarea")
        .context("Failed to wait for edit form")?;

    eprintln!("[screenshot] Capturing edit screenshot...");
    let edit_path = output_dir.join("edit.png");
    let edit_png = tab
        .capture_screenshot(
            headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png,
            None,
            None,
            true,
        )
        .context("Failed to capture edit screenshot")?;
    std::fs::write(&edit_path, &edit_png).context("Failed to write edit screenshot")?;
    eprintln!("[screenshot] Saved: {}", edit_path.display());

    eprintln!("[screenshot] All screenshots captured successfully!");
    Ok(())
}
