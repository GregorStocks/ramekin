import UIKit
import XCTest
@testable import Ramekin

/// Runs shared-test-vectors/auth-session.json, which the web client's
/// authSession.test.ts also runs: a 401 only expires the session whose token
/// it was sent with.
final class AuthSessionSupportTests: XCTestCase {
    private struct VectorState: Decodable {
        let kind: String
        let token: String?

        var authState: AuthState? {
            switch kind {
            case "signedOut": return .signedOut
            case "expired": return .expired
            case "signedIn": return token.map { .signedIn(token: $0) }
            default: return nil
            }
        }
    }

    private struct RejectionVector: Decodable {
        let name: String
        let state: VectorState
        let rejectedToken: String?
        let expected: VectorState
    }

    private struct BearerVector: Decodable {
        let header: String?
        let expected: String?
    }

    private struct StatusVector: Decodable {
        let status: Int
        let expected: Bool
    }

    private struct Vectors: Decodable {
        let afterRejection: [RejectionVector]
        let bearerToken: [BearerVector]
        let sessionRejection: [StatusVector]
    }

    private func loadVectors() throws -> Vectors {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "auth-session", withExtension: "json"))
        return try JSONDecoder().decode(Vectors.self, from: Data(contentsOf: url))
    }

    func testAfterRejectionMatchesSharedVectors() throws {
        let vectors = try loadVectors()
        XCTAssertFalse(vectors.afterRejection.isEmpty)
        for vector in vectors.afterRejection {
            let state = try XCTUnwrap(vector.state.authState, vector.name)
            let expected = try XCTUnwrap(vector.expected.authState, vector.name)
            XCTAssertEqual(state.afterRejection(of: vector.rejectedToken), expected, vector.name)
        }
    }

    func testBearerTokenMatchesSharedVectors() throws {
        for vector in try loadVectors().bearerToken {
            XCTAssertEqual(
                AuthExpiry.bearerToken(fromAuthorizationHeader: vector.header),
                vector.expected,
                "header: \(vector.header ?? "nil")"
            )
        }
    }

    func testSessionRejectionMatchesSharedVectors() throws {
        for vector in try loadVectors().sessionRejection {
            XCTAssertEqual(AuthExpiry.isSessionRejection(statusCode: vector.status), vector.expected, "\(vector.status)")
        }
    }

    func testOnlySignedInCountsAsSignedIn() {
        XCTAssertTrue(AuthState.signedIn(token: "a").isSignedIn)
        XCTAssertFalse(AuthState.signedOut.isSignedIn)
        XCTAssertFalse(AuthState.expired.isSignedIn)
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
