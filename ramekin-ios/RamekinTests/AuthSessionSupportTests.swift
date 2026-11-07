import UIKit
import XCTest
@testable import Ramekin

/// Mirrors ramekin-ui/src/context/authState.test.ts: a 401 only expires the
/// session whose token it was sent with.
final class AuthSessionSupportTests: XCTestCase {
    func testRejectionOfTheCurrentTokenExpiresTheSession() {
        XCTAssertEqual(AuthState.signedIn(token: "a").afterRejection(of: "a"), .expired)
    }

    func testRejectionOfAnEarlierTokenLeavesTheNewSessionAlone() {
        XCTAssertEqual(AuthState.signedIn(token: "b").afterRejection(of: "a"), .signedIn(token: "b"))
    }

    func testRejectionWhileSignedOutOrExpiredChangesNothing() {
        XCTAssertEqual(AuthState.signedOut.afterRejection(of: "a"), .signedOut)
        XCTAssertEqual(AuthState.expired.afterRejection(of: "a"), .expired)
    }

    func testOnlySignedInCountsAsSignedIn() {
        XCTAssertTrue(AuthState.signedIn(token: "a").isSignedIn)
        XCTAssertFalse(AuthState.signedOut.isSignedIn)
        XCTAssertFalse(AuthState.expired.isSignedIn)
    }

    func testBearerTokenParsing() {
        XCTAssertEqual(AuthExpiry.bearerToken(fromAuthorizationHeader: "Bearer abc"), "abc")
        XCTAssertNil(AuthExpiry.bearerToken(fromAuthorizationHeader: "Bearer "))
        XCTAssertNil(AuthExpiry.bearerToken(fromAuthorizationHeader: "Basic abc"))
        XCTAssertNil(AuthExpiry.bearerToken(fromAuthorizationHeader: nil))
    }

    func testOnly401IsASessionRejection() {
        XCTAssertTrue(AuthExpiry.isSessionRejection(statusCode: 401))
        XCTAssertFalse(AuthExpiry.isSessionRejection(statusCode: 403))
        XCTAssertFalse(AuthExpiry.isSessionRejection(statusCode: 500))
    }

    func testNotifyPostsTheRejectedTokenOn401() {
        let center = NotificationCenter()
        var received: [String?] = []
        let observer = center.addObserver(forName: .ramekinAuthExpired, object: nil, queue: nil) {
            received.append(AuthExpiry.rejectedToken(from: $0))
        }
        defer { center.removeObserver(observer) }

        AuthExpiry.notifyIfRejected(statusCode: 401, sentToken: "tok", center: center)
        AuthExpiry.notifyIfRejected(statusCode: 403, sentToken: "tok", center: center)
        AuthExpiry.notifyIfRejected(statusCode: 500, sentToken: "tok", center: center)
        AuthExpiry.notifyIfRejected(statusCode: 401, sentToken: nil, center: center)

        XCTAssertEqual(received, ["tok"])
    }

    @MainActor
    func testImageLoaderReportsA401WithTheTokenItSent() async throws {
        let url = try XCTUnwrap(URL(string: "https://example.com/api/photos/x/thumbnail"))
        let expiry = expectation(forNotification: .ramekinAuthExpired, object: nil) { notification in
            AuthExpiry.rejectedToken(from: notification) == "image-token"
        }
        let loader = AuthenticatedImageLoader(
            imageCache: AuthenticatedImageCache(),
            tokenProvider: { "image-token" },
            imageFetcher: { request in
                let response = HTTPURLResponse(
                    url: try XCTUnwrap(request.url),
                    statusCode: 401,
                    httpVersion: nil,
                    headerFields: nil
                )
                return (Data(), try XCTUnwrap(response))
            }
        )

        loader.load(url: url)

        await fulfillment(of: [expiry], timeout: 2)
    }
}
