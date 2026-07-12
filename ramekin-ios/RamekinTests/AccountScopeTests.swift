import XCTest
@testable import Ramekin

final class AccountScopeTests: XCTestCase {
    func testUsernameCasingUsesSameAccountKey() {
        XCTAssertEqual(
            AccountScope.key(serverURL: "https://example.test", username: "Chef"),
            AccountScope.key(serverURL: "https://example.test", username: "chef")
        )
    }

    func testDifferentServersRemainSeparateAccounts() {
        XCTAssertNotEqual(
            AccountScope.key(serverURL: "https://one.test", username: "chef"),
            AccountScope.key(serverURL: "https://two.test", username: "chef")
        )
    }
}
