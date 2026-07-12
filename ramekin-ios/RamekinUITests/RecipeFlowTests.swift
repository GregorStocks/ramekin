import XCTest

final class RecipeFlowTests: XCTestCase {

    /// Budget for waits that are normally sub-second but must survive a
    /// degraded CI simulator, which can take tens of seconds to service a
    /// single accessibility-hierarchy snapshot. Any timeout shorter than a
    /// couple of slow snapshots flakes on such runners.
    let slowSimulatorTimeout: TimeInterval = 60

    var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        // Keychain state survives reinstalls on the simulator; make the app
        // clear credentials so every test starts from the login screen.
        app.launchArguments = ["--uitest-reset-auth"]
        app.launch()
    }

    override func tearDownWithError() throws {
        app = nil
    }

    /// Clear a text field by triple-tapping to select all, then deleting.
    /// More reliable than long-press + "Select All" menu item on CI.
    private func clearField(_ field: XCUIElement) {
        field.tap()
        field.tap(withNumberOfTaps: 3, numberOfTouches: 1)
        field.typeText(XCUIKeyboardKey.delete.rawValue)
    }

    /// Attach a screenshot of the current app state to the test results.
    private func attachScreenshot(named name: String) {
        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.name = name
        screenshot.lifetime = .keepAlways
        add(screenshot)
    }

    /// Test the full recipe flow: login -> recipe list -> recipe detail
    func testRecipeFlow() throws {
        // MARK: - Login

        // Find and fill server URL field (clear default value first)
        let serverField = app.textFields["https://ramekin.app"]
        XCTAssertTrue(
            serverField.waitForExistence(timeout: slowSimulatorTimeout),
            "Server URL field should exist"
        )
        clearField(serverField)
        serverField.typeText("http://localhost:55000")

        // Find and fill username field (clear default value first)
        let usernameField = app.textFields["Username"]
        XCTAssertTrue(usernameField.exists, "Username field should exist")
        clearField(usernameField)
        usernameField.typeText("t")

        // Find and fill password field (clear default value first)
        let passwordField = app.secureTextFields["Password"]
        XCTAssertTrue(passwordField.exists, "Password field should exist")
        clearField(passwordField)
        passwordField.typeText("t")

        attachScreenshot(named: "01-LoginForm")

        // Tap Sign In button
        let signInButton = app.buttons["Sign In"]
        XCTAssertTrue(signInButton.exists, "Sign In button should exist")
        signInButton.tap()

        // MARK: - Recipe List

        // The login screen is a Form whose rows also match `app.cells`, so the
        // logged-in check must be something only the recipe list has: its
        // "Recipes" navigation bar (the login screen's bar is "Sign In").
        guard app.navigationBars["Recipes"].waitForExistence(timeout: slowSimulatorTimeout) else {
            attachScreenshot(named: "02-AfterLogin")
            XCTFail("Never left the login screen: Recipes navigation bar did not appear after Sign In.")
            return
        }

        // Wait for recipe rows to load (requires seeded data from make seed).
        // A fresh install must first sync the full seed corpus (~475 recipes)
        // into the local cache before any rows render, and that takes well
        // over 15 seconds on CI hardware.
        let recipeCell = app.cells.firstMatch
        guard recipeCell.waitForExistence(timeout: 60) else {
            attachScreenshot(named: "02-EmptyRecipeList")
            XCTFail("Recipe list has no rows. Seed data from make seed is required for UI tests.")
            return
        }
        attachScreenshot(named: "02-RecipeList")

        // MARK: - Recipe Detail

        // Tap first recipe
        recipeCell.tap()

        // Wait for detail view to load: the back button labeled with the
        // previous screen's title only exists on the detail screen
        let backButton = app.navigationBars.buttons["Recipes"]
        XCTAssertTrue(
            backButton.waitForExistence(timeout: slowSimulatorTimeout),
            "Recipe detail view did not appear after tapping a recipe."
        )
        attachScreenshot(named: "03-RecipeDetail")
    }

    /// Test that login fails with invalid credentials
    func testLoginFailure() throws {
        let serverField = app.textFields["https://ramekin.app"]
        XCTAssertTrue(serverField.waitForExistence(timeout: slowSimulatorTimeout))
        clearField(serverField)
        serverField.typeText("http://localhost:55000")

        let usernameField = app.textFields["Username"]
        clearField(usernameField)
        usernameField.typeText("invalid")

        let passwordField = app.secureTextFields["Password"]
        clearField(passwordField)
        passwordField.typeText("wrong")

        app.buttons["Sign In"].tap()

        // The error message renders in the same UI update that ends the
        // in-flight spinner, so it appearing IS the "login request finished"
        // signal. On a healthy run it shows up in well under a second; the
        // budget only has to outlast the app's 15s login timeout plus the
        // tens of seconds a degraded CI simulator can take per accessibility
        // snapshot (issue: testLoginFailure flaked when a fixed 10s expired
        // during a single slow snapshot).
        let errorMessage = app.staticTexts["login-error-message"]
        let errorExists = errorMessage.waitForExistence(timeout: slowSimulatorTimeout)
        attachScreenshot(named: "LoginError")
        if !errorExists {
            if app.descendants(matching: .any)["login-in-progress"].exists {
                XCTFail(
                    "Login request still in flight after \(Int(slowSimulatorTimeout))s "
                        + "despite the app's 15s login timeout — test-host or network stall, "
                        + "not a login-error-path regression."
                )
            } else {
                XCTFail("Login request finished but no error message was shown after a failed login.")
            }
        }
    }
}
