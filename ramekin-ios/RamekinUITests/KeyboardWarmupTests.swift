import XCTest

/// Not a real test: CI runs it on its own (make ios-warm-ui-keyboard), with
/// failures tolerated, before the UI tests, and ios-test-ui-prebuilt skips it.
///
/// The simulator's keyboard runs out of process (InputUI) and is launched on
/// the first keyboard use of the run. On a loaded CI runner, runningboardd
/// suspended that freshly launched process while the keyboard was on screen,
/// and the app's main thread then blocked in UIKeyboardTaskQueue for about
/// four minutes, failing the first test that typed (run 37389551049). Showing
/// the keyboard here moves that cold launch out of the real tests. Since then,
/// no stall has followed a warm launch. When this test failed, it was always
/// the first-launch app hang (issues/p3-ios-ui-app-launch-hang.json5), which
/// left the keyboard cold, so make ios-warm-ui-keyboard retries it once.
final class KeyboardWarmupTests: XCTestCase {

    /// Long enough to outlast the four-minute stall seen on CI.
    let keyboardTimeout: TimeInterval = 300

    func testWarmUpKeyboard() throws {
        let app = XCUIApplication()
        app.launchArguments = ["--uitest-reset-auth"]
        app.launch()

        let serverField = app.textFields["https://ramekin.app"]
        XCTAssertTrue(
            serverField.waitForExistence(timeout: keyboardTimeout),
            "Server URL field should exist"
        )
        serverField.tap()
        XCTAssertTrue(
            app.keyboards.firstMatch.waitForExistence(timeout: keyboardTimeout),
            "Keyboard never appeared"
        )
        serverField.typeText("x")
    }
}
