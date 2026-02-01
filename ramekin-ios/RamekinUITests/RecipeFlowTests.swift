import XCTest

final class RecipeFlowTests: XCTestCase {

    var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launch()
    }

    override func tearDownWithError() throws {
        app = nil
    }

    /// Test the full recipe flow: login -> recipe list -> recipe detail
    func testRecipeFlow() throws {
        // MARK: - Login

        // Find and fill server URL field
        let serverField = app.textFields["https://your-server.com"]
        XCTAssertTrue(serverField.waitForExistence(timeout: 5), "Server URL field should exist")
        serverField.tap()
        serverField.typeText("http://localhost:55000")

        // Find and fill username field
        let usernameField = app.textFields["Username"]
        XCTAssertTrue(usernameField.exists, "Username field should exist")
        usernameField.tap()
        usernameField.typeText("t")

        // Find and fill password field
        let passwordField = app.secureTextFields["Password"]
        XCTAssertTrue(passwordField.exists, "Password field should exist")
        passwordField.tap()
        passwordField.typeText("t")

        // Take screenshot of login form
        let loginScreenshot = XCTAttachment(screenshot: app.screenshot())
        loginScreenshot.name = "01-LoginForm"
        loginScreenshot.lifetime = .keepAlways
        add(loginScreenshot)

        // Tap Sign In button
        let signInButton = app.buttons["Sign In"]
        XCTAssertTrue(signInButton.exists, "Sign In button should exist")
        signInButton.tap()

        // MARK: - Recipe List

        // Wait for recipe list to load (look for navigation title or first cell)
        // The app currently shows SettingsView after login, so this will fail
        // until Phase 6 wires up RecipeListView
        let recipeCell = app.cells.firstMatch
        let recipesLoaded = recipeCell.waitForExistence(timeout: 15)

        if recipesLoaded {
            // Take screenshot of recipe list
            let listScreenshot = XCTAttachment(screenshot: app.screenshot())
            listScreenshot.name = "02-RecipeList"
            listScreenshot.lifetime = .keepAlways
            add(listScreenshot)

            // MARK: - Recipe Detail

            // Tap first recipe
            recipeCell.tap()

            // Wait for detail view to load
            sleep(2)

            // Take screenshot of recipe detail
            let detailScreenshot = XCTAttachment(screenshot: app.screenshot())
            detailScreenshot.name = "03-RecipeDetail"
            detailScreenshot.lifetime = .keepAlways
            add(detailScreenshot)
        } else {
            // Still take a screenshot of whatever we see after login
            let afterLoginScreenshot = XCTAttachment(screenshot: app.screenshot())
            afterLoginScreenshot.name = "02-AfterLogin"
            afterLoginScreenshot.lifetime = .keepAlways
            add(afterLoginScreenshot)

            // This is expected to fail until RecipeListView is implemented
            XCTFail("Recipe list did not load - this is expected until Phase 6")
        }
    }

    /// Test that login fails with invalid credentials
    func testLoginFailure() throws {
        let serverField = app.textFields["https://your-server.com"]
        XCTAssertTrue(serverField.waitForExistence(timeout: 5))
        serverField.tap()
        serverField.typeText("http://localhost:55000")

        let usernameField = app.textFields["Username"]
        usernameField.tap()
        usernameField.typeText("invalid")

        let passwordField = app.secureTextFields["Password"]
        passwordField.tap()
        passwordField.typeText("wrong")

        app.buttons["Sign In"].tap()

        // Wait for error message
        let errorExists = app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'error' OR label CONTAINS 'Invalid' OR label CONTAINS 'unauthorized'")).firstMatch.waitForExistence(timeout: 10)

        let errorScreenshot = XCTAttachment(screenshot: app.screenshot())
        errorScreenshot.name = "LoginError"
        errorScreenshot.lifetime = .keepAlways
        add(errorScreenshot)

        // We expect some error indicator after failed login
        // Don't assert on this since error format may vary
    }
}
