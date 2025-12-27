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
    std::fs::create_dir_all(output_dir).context("Failed to create output directory")?;

    // Launch browser with no-sandbox for Linux compatibility
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
    let tab = browser.new_tab().context("Failed to create tab")?;

    // Set viewport size
    tab.set_bounds(headless_chrome::types::Bounds::Normal {
        left: Some(0),
        top: Some(0),
        width: Some(width as f64),
        height: Some(height as f64),
    })
    .context("Failed to set viewport")?;

    // Navigate to login page
    tab.navigate_to(ui_url)
        .context("Failed to navigate to UI")?;
    tab.wait_for_element("input[type='text']")
        .context("Failed to find username input")?;

    // Log in
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
    tab.wait_for_element("button[type='submit']")
        .context("Failed to find submit button")?
        .click()
        .context("Failed to click submit")?;

    // Wait for cookbook page to load
    tab.wait_for_element(".recipe-card")
        .context("Failed to wait for recipe cards")?;

    // Screenshot 1: Cookbook page
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
    println!("Screenshot saved to {}", cookbook_path.display());

    // Click on first recipe card
    tab.wait_for_element(".recipe-card")
        .context("Failed to find recipe card")?
        .click()
        .context("Failed to click recipe card")?;
    // Wait for recipe page to load (instructions section is always present)
    tab.wait_for_element(".instructions")
        .context("Failed to wait for recipe page")?;
    // Give photos time to load if they exist
    std::thread::sleep(std::time::Duration::from_millis(500));

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
    println!("Screenshot saved to {}", recipe_path.display());

    // Screenshot 3: Edit page
    let recipe_url = tab.get_url();
    let edit_url = format!("{}/edit", recipe_url);
    tab.navigate_to(&edit_url)
        .context("Failed to navigate to edit page")?;
    tab.wait_for_element("textarea")
        .context("Failed to wait for edit form")?;

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
    println!("Screenshot saved to {}", edit_path.display());

    Ok(())
}
