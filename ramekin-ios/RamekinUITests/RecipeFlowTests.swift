import XCTest

final class RecipeFlowTests: XCTestCase {
    private let importedRecipeTitle = "Armenian-Style Rice Pilaf"

    var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        if name.contains("testRecipeFlow") {
            app.launchEnvironment["UITEST_RECIPE_SEARCH"] = importedRecipeTitle
        }
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

    private func openRecipeFromList(named title: String, maxSwipes: Int = 40) -> Bool {
        let recipeTitle = app.staticTexts[title]
        if recipeTitle.waitForExistence(timeout: 2) {
            recipeTitle.tap()
            return true
        }

        for _ in 0..<maxSwipes {
            app.swipeUp()
            if recipeTitle.waitForExistence(timeout: 1) {
                recipeTitle.tap()
                return true
            }
        }

        return false
    }

    private func scrollToStaticText(
        containing text: String,
        maxSwipes: Int = 6
    ) -> XCUIElement? {
        let predicate = NSPredicate(format: "label CONTAINS[c] %@", text)
        let matchingText = app.staticTexts.matching(predicate).firstMatch

        if matchingText.waitForExistence(timeout: 2) {
            return matchingText
        }

        for _ in 0..<maxSwipes {
            app.swipeUp()
            if matchingText.waitForExistence(timeout: 1) {
                return matchingText
            }
        }

        return nil
    }

    /// Test the full recipe flow: login -> recipe list -> recipe detail
    func testRecipeFlow() throws {
        // MARK: - Login

        // Find and fill server URL field (clear default value first)
        let serverField = app.textFields["https://media.noodles:5173"]
        XCTAssertTrue(serverField.waitForExistence(timeout: 5), "Server URL field should exist")
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

        // Wait for recipe list to load (requires seeded data from make seed)
        let recipeCell = app.cells.firstMatch
        let recipesLoaded = recipeCell.waitForExistence(timeout: 15)

        if recipesLoaded {
            // Take screenshot of recipe list
            let listScreenshot = XCTAttachment(screenshot: app.screenshot())
            listScreenshot.name = "02-RecipeList"
            listScreenshot.lifetime = .keepAlways
            add(listScreenshot)

            // MARK: - Recipe Detail

            XCTAssertTrue(
                openRecipeFromList(named: importedRecipeTitle),
                "Seeded recipe with source URL should exist somewhere in the recipe list"
            )

            // Wait for detail view to load
            sleep(2)

            // Take screenshot of recipe detail
            let detailScreenshot = XCTAttachment(screenshot: app.screenshot())
            detailScreenshot.name = "03-RecipeDetail"
            detailScreenshot.lifetime = .keepAlways
            add(detailScreenshot)

            let rescrapeButton = app.buttons["Rescrape"]
            XCTAssertTrue(
                rescrapeButton.waitForExistence(timeout: 5),
                "Rescrape action should exist for imported recipes with a source URL"
            )
        } else {
            // Still take a screenshot of whatever we see after login
            let afterLoginScreenshot = XCTAttachment(screenshot: app.screenshot())
            afterLoginScreenshot.name = "02-AfterLogin"
            afterLoginScreenshot.lifetime = .keepAlways
            add(afterLoginScreenshot)

            XCTFail("Recipe list did not load. Seed data from make seed is required for UI tests.")
        }
    }

    /// Test that login fails with invalid credentials
    func testLoginFailure() throws {
        let serverField = app.textFields["https://media.noodles:5173"]
        XCTAssertTrue(serverField.waitForExistence(timeout: 5))
        clearField(serverField)
        serverField.typeText("http://localhost:55000")

        let usernameField = app.textFields["Username"]
        clearField(usernameField)
        usernameField.typeText("invalid")

        let passwordField = app.secureTextFields["Password"]
        clearField(passwordField)
        passwordField.typeText("wrong")

        app.buttons["Sign In"].tap()

        // Wait for error message
        let errorExists = scrollToStaticText(containing: "Invalid credentials") != nil

        let errorScreenshot = XCTAttachment(screenshot: app.screenshot())
        errorScreenshot.name = "LoginError"
        errorScreenshot.lifetime = .keepAlways
        add(errorScreenshot)

        XCTAssertTrue(errorExists, "Expected an error message after failed login.")
    }
}
