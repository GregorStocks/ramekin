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
    tracing::debug!("Starting screenshot capture");
    tracing::debug!(url = %ui_url, "UI URL");
    tracing::debug!(path = %output_dir.display(), "Output directory");
    tracing::debug!(width, height, "Viewport size");

    std::fs::create_dir_all(output_dir).context("Failed to create output directory")?;
    tracing::debug!("Output directory created/verified");

    // Launch browser with no-sandbox for Linux compatibility
    tracing::debug!("Launching headless Chrome...");
    let browser = Browser::new(
        headless_chrome::LaunchOptions::default_builder()
            .args(vec![
                std::ffi::OsStr::new("--no-sandbox"),
                std::ffi::OsStr::new("--disable-dev-shm-usage"),
                std::ffi::OsStr::new("--ignore-certificate-errors"),
            ])
            .build()
            .expect("Failed to build launch options"),
    )
    .context("Failed to launch browser")?;
    tracing::debug!("Browser launched successfully");

    let tab = browser.new_tab().context("Failed to create tab")?;
    tracing::debug!("New tab created");

    // Set viewport size
    tab.set_bounds(headless_chrome::types::Bounds::Normal {
        left: Some(0),
        top: Some(0),
        width: Some(width as f64),
        height: Some(height as f64),
    })
    .context("Failed to set viewport")?;
    tracing::debug!(width, height, "Viewport set");

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

    // Screenshot 1: Cookbook page
    tracing::debug!("Capturing cookbook screenshot...");
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
    tracing::debug!(path = %cookbook_path.display(), "Saved cookbook screenshot");

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

    tracing::debug!("Capturing recipe screenshot...");
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
    tracing::debug!(path = %recipe_path.display(), "Saved recipe screenshot");

    // Screenshot 3: Edit page
    let recipe_url = tab.get_url();
    let edit_url = format!("{}/edit", recipe_url);
    tracing::debug!(url = %edit_url, "Navigating to edit page...");
    tab.navigate_to(&edit_url)
        .context("Failed to navigate to edit page")?;
    tracing::debug!("Waiting for edit form (looking for textarea)...");
    tab.wait_for_element("textarea")
        .context("Failed to wait for edit form")?;

    tracing::debug!("Capturing edit screenshot...");
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
    tracing::debug!(path = %edit_path.display(), "Saved edit screenshot");

    tracing::info!("All screenshots captured successfully");
    Ok(())
}
