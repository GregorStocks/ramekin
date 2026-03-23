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

    /// Clear a text field by triple-tapping to select all, then deleting.
    /// More reliable than long-press + "Select All" menu item on CI.
    private func clearField(_ field: XCUIElement) {
        field.tap()
        field.tap(withNumberOfTaps: 3, numberOfTouches: 1)
        field.typeText(XCUIKeyboardKey.delete.rawValue)
    }

    private func login() {
        let serverField = app.textFields["https://media.noodles:5173"]
        XCTAssertTrue(serverField.waitForExistence(timeout: 5), "Server URL field should exist")
        clearField(serverField)
        serverField.typeText("http://localhost:55000")

        let usernameField = app.textFields["Username"]
        XCTAssertTrue(usernameField.exists, "Username field should exist")
        clearField(usernameField)
        usernameField.typeText("t")

        let passwordField = app.secureTextFields["Password"]
        XCTAssertTrue(passwordField.exists, "Password field should exist")
        clearField(passwordField)
        passwordField.typeText("t")

        let loginScreenshot = XCTAttachment(screenshot: app.screenshot())
        loginScreenshot.name = "01-LoginForm"
        loginScreenshot.lifetime = .keepAlways
        add(loginScreenshot)

        app.buttons["Sign In"].tap()
    }

    /// Wait for the recipe list to finish loading after login.
    /// Uses recipe-row accessibility identifiers which are unique to actual recipe rows,
    /// avoiding false matches on login form cells.
    private func waitForRecipeList(timeout: TimeInterval = 15) -> Bool {
        let recipeRow = app.descendants(matching: .any)
            .matching(NSPredicate(format: "identifier BEGINSWITH 'recipe-row-'"))
            .firstMatch
        return recipeRow.waitForExistence(timeout: timeout)
    }

    /// Find a recipe with a Rescrape action by tapping recipe rows one at a time.
    /// Uses accessibility identifiers on NavigationLink elements rather than
    /// generic cell queries, which avoids matching non-recipe cells
    /// (login form, filter bar, load-more indicator).
    private func openRecipeWithRescrapeAction(maxAttempts: Int = 20) -> Bool {
        for attempt in 0..<maxAttempts {
            // Get all recipe rows currently in the accessibility tree
            let allRows = app.descendants(matching: .any)
                .matching(NSPredicate(format: "identifier BEGINSWITH 'recipe-row-'"))

            guard attempt < allRows.count else {
                // Scroll to load more recipes
                app.swipeUp()
                sleep(1)
                continue
            }

            let row = allRows.element(boundBy: attempt)
            guard row.exists && row.isHittable else {
                // Row is off-screen, scroll it into view
                app.swipeUp()
                continue
            }

            row.tap()

            let rescrapeButton = app.buttons["Rescrape"]
            if rescrapeButton.waitForExistence(timeout: 5) {
                return true
            }

            // Navigate back — try known back button labels, then fallback
            let wentBack = app.navigationBars.buttons["Recipes"].exists
                || app.navigationBars.buttons["Back"].exists
            if wentBack {
                let btn = app.navigationBars.buttons["Recipes"].exists
                    ? app.navigationBars.buttons["Recipes"]
                    : app.navigationBars.buttons["Back"]
                btn.tap()
                _ = app.descendants(matching: .any)
                    .matching(NSPredicate(format: "identifier BEGINSWITH 'recipe-row-'"))
                    .firstMatch
                    .waitForExistence(timeout: 5)
            }
        }

        return false
    }

    /// Test the full recipe flow: login -> recipe list -> recipe detail
    func testRecipeFlow() throws {
        // MARK: - Login
        login()

        // MARK: - Recipe List
        let recipesLoaded = waitForRecipeList()

        if recipesLoaded {
            let listScreenshot = XCTAttachment(screenshot: app.screenshot())
            listScreenshot.name = "02-RecipeList"
            listScreenshot.lifetime = .keepAlways
            add(listScreenshot)

            // MARK: - Recipe Detail
            XCTAssertTrue(
                openRecipeWithRescrapeAction(),
                "At least one recipe in the seeded list should expose the rescrape action"
            )

            sleep(2)

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

        let errorElement = app.descendants(matching: .any)
            .matching(identifier: "login-error-message")
            .firstMatch
        let errorExists = errorElement.waitForExistence(timeout: 5)

        let errorScreenshot = XCTAttachment(screenshot: app.screenshot())
        errorScreenshot.name = "LoginError"
        errorScreenshot.lifetime = .keepAlways
        add(errorScreenshot)

        XCTAssertTrue(errorExists, "Expected an error message after failed login.")
    }
}
