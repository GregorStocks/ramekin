import Foundation

extension Notification.Name {
    /// Posted when a bearer-authenticated request returns 401. The rejected
    /// token is in `userInfo[AuthExpiry.rejectedTokenKey]`, so observers end
    /// only the session that sent it, not one signed in since.
    static let ramekinAuthExpired = Notification.Name("ramekinAuthExpired")
}

/// The app's sign-in state. A session ends as `.expired` when the server
/// rejects its token, so the login screen can say why the user is there.
enum AuthState: Equatable {
    case signedOut
    case signedIn(token: String)
    case expired

    var isSignedIn: Bool {
        if case .signedIn = self { return true }
        return false
    }

    /// The state after the server rejected a request sent with
    /// `rejectedToken`. A late 401 for an earlier session's token, or for a
    /// request sent without one, leaves the current session alone. Pinned by
    /// shared-test-vectors/auth-session.json.
    func afterRejection(of rejectedToken: String?) -> AuthState {
        if case .signedIn(let token) = self, token == rejectedToken {
            return .expired
        }
        return self
    }
}

/// Signals that the server rejected a session token. Every request path that
/// sends a bearer token (generated client, `performRequest`, image loads)
/// reports through here.
enum AuthExpiry {
    static let rejectedTokenKey = "rejectedToken"

    /// Only 401 means the session is gone. 403 means the request was refused
    /// for another reason (a route the token may not use, or a Cloudflare
    /// Access policy), and signing in again would not help.
    static func isSessionRejection(statusCode: Int) -> Bool {
        statusCode == 401
    }

    /// The token in an `Authorization: Bearer <token>` header value.
    static func bearerToken(fromAuthorizationHeader header: String?) -> String? {
        guard let header else { return nil }
        let prefix = "Bearer "
        guard header.hasPrefix(prefix) else { return nil }
        let token = String(header.dropFirst(prefix.count))
        return token.isEmpty ? nil : token
    }

    /// Post `.ramekinAuthExpired` when a request sent with `sentToken` came
    /// back 401. Requests sent without a token (login, signup) never post.
    static func notifyIfRejected(
        statusCode: Int,
        sentToken: String?,
        center: NotificationCenter = .default
    ) {
        guard isSessionRejection(statusCode: statusCode), let sentToken else { return }
        center.post(
            name: .ramekinAuthExpired,
            object: nil,
            userInfo: [rejectedTokenKey: sentToken]
        )
    }

    static func rejectedToken(from notification: Notification) -> String? {
        notification.userInfo?[rejectedTokenKey] as? String
    }
}
