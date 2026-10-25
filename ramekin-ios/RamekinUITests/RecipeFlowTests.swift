import XCTest

final class RecipeFlowTests: XCTestCase {

    /// Budget for waits that are normally sub-second; CI simulators are slow.
    let slowSimulatorTimeout: TimeInterval = 30

    /// The seeded backend CI starts for these tests.
    let serverURL = "http://localhost:55000"

    var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        // Keychain state survives reinstalls on the simulator; make the app
        // clear stored credentials at launch. Each test launches the app
        // itself: at the login screen, or with a session (launchSignedIn).
        app.launchArguments = ["--uitest-reset-auth"]
    }

    override func tearDownWithError() throws {
        app = nil
    }

    /// Launch already signed in as the seeded user and wait for the recipe
    /// list.
    private func launchSignedIn(file: StaticString = #filePath, line: UInt = #line) throws {
        let token = try signInOverHTTP(username: "t", password: "t")
        app.launchEnvironment["RAMEKIN_UITEST_SERVER_URL"] = serverURL
        app.launchEnvironment["RAMEKIN_UITEST_TOKEN"] = token
        app.launchEnvironment["RAMEKIN_UITEST_USERNAME"] = "t"
        app.launch()
        XCTAssertTrue(
            app.navigationBars["Recipes"].waitForExistence(timeout: slowSimulatorTimeout),
            "Launching with a session should open the recipe list",
            file: file,
            line: line
        )
    }

    /// Sign in against the test backend directly and return the session token.
    private func signInOverHTTP(username: String, password: String) throws -> String {
        let url = try XCTUnwrap(URL(string: "\(serverURL)/api/auth/login"))
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONSerialization.data(
            withJSONObject: ["username": username, "password": password]
        )
        let response = HTTPResponseBox()
        let finished = expectation(description: "Sign in over HTTP")
        URLSession.shared.dataTask(with: request) { data, urlResponse, error in
            response.data = data
            response.status = (urlResponse as? HTTPURLResponse)?.statusCode
            response.error = error
            finished.fulfill()
        }.resume()
        wait(for: [finished], timeout: slowSimulatorTimeout)
        if let error = response.error {
            throw error
        }
        XCTAssertEqual(response.status, 200, "Sign in over HTTP failed")
        let body = try JSONSerialization.jsonObject(with: try XCTUnwrap(response.data))
        return try XCTUnwrap((body as? [String: Any])?["token"] as? String)
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

    /// Test the recipe flow: recipe list -> recipe detail -> sheets
    func testRecipeFlow() throws {
        try launchSignedIn()

        // MARK: - Recipe List

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

    /// iOS terminating the app in the background must not lose the user's
    /// place: relaunching restores the tab, the open recipe, and its scale.
    func testRelaunchAfterTerminationRestoresPlace() throws {
        try launchSignedIn()
        let recipeCell = app.cells.firstMatch
        XCTAssertTrue(recipeCell.waitForExistence(timeout: 60), "Seed data from make seed is required")
        recipeCell.tap()
        XCTAssertTrue(app.navigationBars.buttons["Edit Recipe"].waitForExistence(timeout: slowSimulatorTimeout))
        let recipeTitle = app.navigationBars.firstMatch.identifier

        let doubleScale = app.buttons["2x"]
        scrollUntilHittable(doubleScale)
        doubleScale.tap()
        XCTAssertTrue(waitUntilSelected(doubleScale), "Tapping 2x should select it")

        app.tabBars.buttons["Meal Plan"].tap()
        XCTAssertTrue(app.navigationBars["Meal Plan"].waitForExistence(timeout: slowSimulatorTimeout))

        // Background first so iOS saves scene state, then kill the process
        // the way the OS does; swiping the app away would discard the state.
        XCUIDevice.shared.press(.home)
        XCTAssertTrue(app.wait(for: .runningBackgroundSuspended, timeout: slowSimulatorTimeout))
        app.terminate()
        // Keep the stored session: resetting auth would sign out.
        app.launchArguments = []
        app.launch()

        XCTAssertTrue(
            app.navigationBars["Meal Plan"].waitForExistence(timeout: slowSimulatorTimeout),
            "Relaunch should restore the selected tab"
        )
        attachScreenshot(named: "08-RestoredTab")
        app.tabBars.buttons["Recipes"].tap()
        XCTAssertTrue(
            app.navigationBars[recipeTitle].waitForExistence(timeout: slowSimulatorTimeout),
            "Relaunch should restore the open recipe"
        )
        XCTAssertTrue(app.navigationBars.buttons["Edit Recipe"].waitForExistence(timeout: slowSimulatorTimeout))
        scrollUntilHittable(doubleScale)
        XCTAssertTrue(waitUntilSelected(doubleScale), "Relaunch should restore the recipe's scale")
        attachScreenshot(named: "09-RestoredRecipe")
    }

    private func scrollUntilHittable(_ element: XCUIElement) {
        XCTAssertTrue(element.waitForExistence(timeout: slowSimulatorTimeout))
        for _ in 0..<8 where !element.isHittable {
            let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.75))
            let end = app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
            start.press(forDuration: 0.01, thenDragTo: end)
        }
    }

    private func waitUntilSelected(_ element: XCUIElement) -> Bool {
        let selected = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "isSelected == true"),
            object: element
        )
        return XCTWaiter.wait(for: [selected], timeout: slowSimulatorTimeout) == .completed
    }
}

/// Carries a URLSession response out of its completion handler.
private final class HTTPResponseBox {
    var data: Data?
    var status: Int?
    var error: Error?
}
