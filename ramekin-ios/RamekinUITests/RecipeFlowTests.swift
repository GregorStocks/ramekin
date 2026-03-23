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
        // Try the back button labeled with the previous view's title
        for label in ["Recipes", "Back"] {
            let btn = app.navigationBars.buttons[label]
            if btn.exists {
                btn.tap()
                return
            }
        }

        // Fallback: any navigation bar button that isn't the ellipsis menu
        let nonMenuButtons = app.navigationBars.buttons.allElementsBoundByIndex.filter { button in
            button.exists && button.identifier != "ellipsis.circle"
        }
        if let backButton = nonMenuButtons.first {
            backButton.tap()
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

                let rescrapeButton = app.buttons["Rescrape"]
                if rescrapeButton.waitForExistence(timeout: 5) {
                    return true
                }

                // Check if we actually navigated to the detail view before going back
                let onDetailView = app.navigationBars.buttons["Recipes"].exists
                    || app.navigationBars.buttons["Back"].exists
                    || app.navigationBars.buttons.matching(identifier: "ellipsis.circle").firstMatch.exists
                guard onDetailView else {
                    // Tap didn't trigger navigation (cell may be partially obscured)
                    continue
                }

                returnToRecipeList()
                XCTAssertTrue(
                    app.cells.firstMatch.waitForExistence(timeout: 5),
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
