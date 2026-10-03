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

    func testTextRecipeReviewAndSave() throws {
        let server = app.textFields["https://ramekin.app"]
        XCTAssertTrue(server.waitForExistence(timeout: slowSimulatorTimeout))
        replaceText(in: server, with: "http://localhost:55000")
        replaceText(in: app.textFields["Username"], with: "t")
        replaceText(in: app.secureTextFields["Password"], with: "t")
        submitLogin()
        XCTAssertTrue(app.navigationBars["Recipes"].waitForExistence(timeout: slowSimulatorTimeout))
        app.buttons["New Recipe"].tap()
        let text = app.textViews["Recipe text"]
        XCTAssertTrue(text.waitForExistence(timeout: slowSimulatorTimeout))
        text.tap()
        text.typeText("Text Pancakes\n1 cup all-purpose flour\nMix and cook.")
        app.swipeUp()
        app.buttons["Review recipe"].tap()
        let title = app.textFields["Recipe title"]
        XCTAssertTrue(title.waitForExistence(timeout: slowSimulatorTimeout))
        replaceText(in: title, with: "iOS text pancakes")
        app.navigationBars.buttons["Save"].tap()
        XCTAssertTrue(app.navigationBars["Recipes"].waitForExistence(timeout: slowSimulatorTimeout))
        let search = app.searchFields.firstMatch
        XCTAssertTrue(search.waitForExistence(timeout: slowSimulatorTimeout))
        search.tap()
        search.typeText("iOS text pancakes")
        let savedRecipe = app.buttons.matching(
            NSPredicate(format: "label BEGINSWITH %@", "iOS text pancakes")
        ).firstMatch
        XCTAssertTrue(savedRecipe.waitForExistence(timeout: slowSimulatorTimeout))
    }

    /// Replace a field's contents, then synchronize on the resulting value.
    ///
    /// Selecting the old text with a triple-tap is unreliable on a slow CI
    /// simulator: it can land as a double-tap that selects only the last word,
    /// or select nothing, and the typed text then joins what was left (a URL
    /// of "https://ramekin.ap" plus the new one, or a recipe saved as
    /// "TextiOS text pancakes"). Instead, put the cursor after the last
    /// character and delete exactly as many characters as the field holds.
    private func replaceText(
        in field: XCUIElement,
        with text: String,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        field.tap()
        // An empty field reports its placeholder as its value.
        let current = field.value as? String ?? ""
        if !current.isEmpty && current != field.placeholderValue {
            // A tap past the end of the text puts the cursor after it.
            field.coordinate(withNormalizedOffset: CGVector(dx: 0.98, dy: 0.5)).tap()
            field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: current.count))
        }
        field.typeText(text)
        // A secure field reports one bullet per character.
        let expected = field.elementType == .secureTextField
            ? String(repeating: "•", count: text.count)
            : text
        assertAccessibleValue(field, equals: expected, file: file, line: line)
    }

    /// Input can finish before a slow simulator's accessibility snapshot
    /// reflects it. Synchronize on the resulting value before continuing.
    private func assertAccessibleValue(
        _ element: XCUIElement,
        equals expected: String,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        let updated = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "value == %@", expected),
            object: element
        )
        XCTAssertEqual(
            XCTWaiter.wait(for: [updated], timeout: slowSimulatorTimeout),
            .completed,
            "Expected accessibility value: \(expected), got: \(String(describing: element.value))",
            file: file,
            line: line
        )
    }

    /// Attach a screenshot of the current app state to the test results.
    private func attachScreenshot(named name: String) {
        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.name = name
        screenshot.lifetime = .keepAlways
        add(screenshot)
    }

    private func submitLogin() {
        // Reveal the final Form rows before tapping. XCTest's implicit
        // scroll-to-visible can move the button while synthesizing its tap
        // on a slow simulator, leaving the login request unsubmitted.
        app.swipeUp()
        let button = app.buttons["Sign In"]
        XCTAssertTrue(button.isEnabled, "Login fields must be populated before submitting")
        XCTAssertTrue(button.isHittable, "Sign In must be visible before tapping")
        button.tap()
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
        replaceText(in: serverField, with: "http://localhost:55000")

        // Find and fill username field (clear default value first)
        let usernameField = app.textFields["Username"]
        XCTAssertTrue(usernameField.exists, "Username field should exist")
        replaceText(in: usernameField, with: "t")

        // Find and fill password field (clear default value first)
        let passwordField = app.secureTextFields["Password"]
        XCTAssertTrue(passwordField.exists, "Password field should exist")
        replaceText(in: passwordField, with: "t")

        attachScreenshot(named: "01-LoginForm")

        // Tap Sign In button
        submitLogin()

        // MARK: - Recipe List

        // The login screen is a Form whose rows also match `app.cells`, so the
        // logged-in check must be something only the recipe list has: its
        // "Recipes" navigation bar (the login screen's bar is "Sign In").
        let recipesBar = app.navigationBars["Recipes"]
        let loginError = app.staticTexts["login-error-message"]
        let loginFinished = XCTNSPredicateExpectation(
            predicate: NSPredicate { _, _ in recipesBar.exists || loginError.exists },
            object: nil
        )
        let loginResult = XCTWaiter.wait(for: [loginFinished], timeout: slowSimulatorTimeout)
        guard loginResult == .completed, recipesBar.exists else {
            attachScreenshot(named: "02-AfterLogin")
            let error = loginError.exists ? loginError.label : "No login error displayed"
            XCTFail("Never reached Recipes after Sign In: \(error).\n\(app.debugDescription)")
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

        let newRecipeButton = app.navigationBars.buttons["New Recipe"]
        XCTAssertTrue(newRecipeButton.isHittable, "New Recipe needs an accessible toolbar action")
        newRecipeButton.tap()
        XCTAssertTrue(app.navigationBars["New Recipe"].waitForExistence(timeout: slowSimulatorTimeout))
        attachScreenshot(named: "02-NewRecipe")
        app.navigationBars.buttons["Cancel"].tap()
        XCTAssertTrue(app.navigationBars["Recipes"].waitForExistence(timeout: slowSimulatorTimeout))

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

        let editButton = app.navigationBars.buttons["Edit Recipe"]
        XCTAssertTrue(editButton.waitForExistence(timeout: slowSimulatorTimeout))
        XCTAssertTrue(editButton.isHittable, "Edit should be directly available without opening More actions")
        XCTAssertTrue(app.navigationBars.buttons["More recipe actions"].isHittable)
        editButton.tap()
        XCTAssertTrue(app.navigationBars["Edit Recipe"].waitForExistence(timeout: slowSimulatorTimeout))
        attachScreenshot(named: "04-EditRecipe")

        let ingredientField = app.textFields["Ingredient"].firstMatch
        let removeIngredient = app.buttons["Remove ingredient"].firstMatch
        for _ in 0..<8 where !removeIngredient.isHittable {
            let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.75))
            let end = app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
            start.press(forDuration: 0.01, thenDragTo: end)
        }
        XCTAssertTrue(ingredientField.isHittable)
        XCTAssertTrue(app.textFields["Amount"].firstMatch.isHittable)
        XCTAssertTrue(app.textFields["Unit"].firstMatch.isHittable)
        XCTAssertTrue(removeIngredient.isHittable)
        XCTAssertGreaterThanOrEqual(removeIngredient.frame.height, 44)
        attachScreenshot(named: "05-IngredientEditor")
        app.navigationBars.buttons["Cancel"].tap()
        XCTAssertTrue(editButton.waitForExistence(timeout: slowSimulatorTimeout))

        // Native sheets must expose their title and controls while keeping
        // the underlying recipe actions out of reach until dismissal.
        let moreActions = app.navigationBars.buttons["More recipe actions"]
        moreActions.tap()
        app.buttons["Add to Shopping List"].tap()
        let shoppingBar = app.navigationBars["Add to Shopping List"]
        XCTAssertTrue(shoppingBar.waitForExistence(timeout: slowSimulatorTimeout))
        XCTAssertFalse(editButton.isHittable)
        let ingredient = app.buttons["shopping-ingredient-0"]
        XCTAssertTrue(ingredient.waitForExistence(timeout: slowSimulatorTimeout))
        XCTAssertFalse(ingredient.label.isEmpty)
        assertAccessibleValue(ingredient, equals: "Selected")
        ingredient.tap()
        assertAccessibleValue(ingredient, equals: "Not selected")
        attachScreenshot(named: "06-ShoppingSheet")
        shoppingBar.buttons["Cancel"].tap()
        XCTAssertTrue(moreActions.waitForExistence(timeout: slowSimulatorTimeout))
        XCTAssertTrue(moreActions.isHittable)

        moreActions.tap()
        app.buttons["Add to Shopping List"].tap()
        XCTAssertTrue(shoppingBar.waitForExistence(timeout: slowSimulatorTimeout))
        assertAccessibleValue(ingredient, equals: "Selected")
        shoppingBar.buttons["Cancel"].tap()
        XCTAssertTrue(moreActions.waitForExistence(timeout: slowSimulatorTimeout))

        moreActions.tap()
        app.buttons["Add to Meal Plan"].tap()
        let mealPlanBar = app.navigationBars["Add to Meal Plan"]
        XCTAssertTrue(mealPlanBar.waitForExistence(timeout: slowSimulatorTimeout))
        XCTAssertFalse(editButton.isHittable)
        XCTAssertTrue(mealPlanBar.buttons["Add"].isHittable)
        attachScreenshot(named: "07-MealPlanSheet")
        mealPlanBar.buttons["Cancel"].tap()
        XCTAssertTrue(moreActions.waitForExistence(timeout: slowSimulatorTimeout))
        XCTAssertTrue(moreActions.isHittable)
    }

    /// Test that login fails with invalid credentials
    func testLoginFailure() throws {
        let serverField = app.textFields["https://ramekin.app"]
        XCTAssertTrue(serverField.waitForExistence(timeout: slowSimulatorTimeout))
        replaceText(in: serverField, with: "http://localhost:55000")
        replaceText(in: app.textFields["Username"], with: "invalid")
        replaceText(in: app.secureTextFields["Password"], with: "wrong")

        submitLogin()

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
