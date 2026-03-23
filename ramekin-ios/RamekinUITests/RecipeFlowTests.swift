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

    private func scrollToElement(
        identifier: String,
        maxSwipes: Int = 6
    ) -> XCUIElement? {
        let matchingElement = app.descendants(matching: .any)
            .matching(identifier: identifier)
            .firstMatch

        if matchingElement.waitForExistence(timeout: 2) {
            return matchingElement
        }

        for _ in 0..<maxSwipes {
            app.swipeUp()
            if matchingElement.waitForExistence(timeout: 1) {
                return matchingElement
            }
        }

        return nil
    }

    private func visibleRecipeCells() -> [XCUIElement] {
        app.cells.allElementsBoundByIndex.filter { cell in
            cell.exists && cell.isHittable && cell.staticTexts.count > 0
        }
    }

    private func recipeCellIdentifier(_ cell: XCUIElement, pageIndex: Int, fallbackIndex: Int) -> String {
        if !cell.identifier.isEmpty {
            return cell.identifier
        }

        let title = cell.staticTexts.firstMatch.label.trimmingCharacters(in: .whitespacesAndNewlines)
        if !title.isEmpty {
            return "recipe-title-\(title)"
        }

        return "page-\(pageIndex)-index-\(fallbackIndex)"
    }

    private func returnToRecipeList() {
        let backButton = app.navigationBars.buttons["Recipes"]
        if backButton.waitForExistence(timeout: 3) {
            backButton.tap()
            return
        }

        // Fallback: try "Back" label (used when the title is too long)
        let genericBack = app.navigationBars.buttons["Back"]
        if genericBack.exists {
            genericBack.tap()
            return
        }

        XCTFail("Could not find a navigation button to return to the recipe list")
    }

    private func openRecipeWithRescrapeAction(maxPages: Int = 8) -> Bool {
        var attemptedRecipeIdentifiers = Set<String>()

        for pageIndex in 0..<maxPages {
            var fallbackIndex = 0

            while true {
                let candidates = visibleRecipeCells().compactMap { cell -> (String, XCUIElement)? in
                    let identifier = recipeCellIdentifier(
                        cell,
                        pageIndex: pageIndex,
                        fallbackIndex: fallbackIndex
                    )

                    guard !attemptedRecipeIdentifiers.contains(identifier) else {
                        return nil
                    }

                    return (identifier, cell)
                }

                guard let (identifier, cell) = candidates.first else {
                    break
                }

                attemptedRecipeIdentifiers.insert(identifier)
                fallbackIndex += 1
                cell.tap()

                // Check if navigation to the detail view occurred (back button appears)
                let backButton = app.navigationBars.buttons["Recipes"]
                guard backButton.waitForExistence(timeout: 3) else {
                    // Tap didn't trigger navigation (cell may be partially obscured)
                    continue
                }

                let rescrapeButton = app.buttons["Rescrape"]
                if rescrapeButton.waitForExistence(timeout: 5) {
                    return true
                }

                returnToRecipeList()
                XCTAssertTrue(
                    app.navigationBars["Recipes"].waitForExistence(timeout: 5),
                    "Recipe list should reappear after returning from detail view"
                )
            }

            app.swipeUp()
        }

        return false
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

        // Wait for the tab bar to appear (login screen has no tabs; recipe list does)
        let recipesTab = app.tabBars.buttons["Recipes"]
        let recipesLoaded = recipesTab.waitForExistence(timeout: 15)

        if recipesLoaded {
            // Take screenshot of recipe list
            let listScreenshot = XCTAttachment(screenshot: app.screenshot())
            listScreenshot.name = "02-RecipeList"
            listScreenshot.lifetime = .keepAlways
            add(listScreenshot)

            // MARK: - Recipe Detail

            XCTAssertTrue(
                openRecipeWithRescrapeAction(),
                "At least one recipe in the seeded list should expose the rescrape action"
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
        let errorExists = scrollToElement(identifier: "login-error-message") != nil

        let errorScreenshot = XCTAttachment(screenshot: app.screenshot())
        errorScreenshot.name = "LoginError"
        errorScreenshot.lifetime = .keepAlways
        add(errorScreenshot)

        XCTAssertTrue(errorExists, "Expected an error message after failed login.")
    }
}
