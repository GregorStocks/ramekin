import Foundation

extension RamekinAPI {
    /// The login request, addressed to `serverURL` and carrying only the
    /// given Access credentials, never the global custom headers (any old
    /// session's token and Access credentials).
    static func loginRequestBuilder(
        serverURL: String,
        loginRequest: LoginRequest,
        accessClientId: String?,
        accessClientSecret: String?
    ) -> RequestBuilder<LoginResponse> {
        let builder = LoginRequestBuilder(
            method: "POST",
            URLString: serverURL + "/api/auth/login",
            parameters: JSONEncodingHelper.encodingParameters(forEncodableObject: loginRequest),
            headers: [:],
            requiresAuthentication: false
        )
        var headers = ["Content-Type": "application/json"]
        headers["CF-Access-Client-Id"] = accessClientId
        headers["CF-Access-Client-Secret"] = accessClientSecret
        // The init copied the global custom headers in; drop them.
        builder.headers = headers
        return builder
    }
}

/// Builds the login request from its own headers only. The generated
/// builder merges `RamekinClientAPI.customHeaders` in again when it creates
/// the URLRequest, which would send an old session's bearer token and Access
/// credentials, possibly to a different server.
final class LoginRequestBuilder: InsecureDecodableBuilder<LoginResponse> {
    override func buildHeaders() -> [String: String] {
        headers
    }
}

/// A signed-in session as the keychain stores it.
struct SessionCredentials {
    let serverURL: String
    let token: String
    let username: String
    let accessClientId: String?
    let accessClientSecret: String?

    /// Write the session. Missing Access credentials clear the stored ones
    /// so stale values aren't re-sent. The token goes last, and if any write
    /// fails it is deleted, so a partly written session never reads as
    /// signed in. Returns whether every write succeeded.
    func save(in store: CredentialStore) -> Bool {
        let saved = store.saveServerURL(serverURL)
            && store.saveUsername(username)
            && Self.save(accessClientId, with: store.saveAccessClientId, orDelete: store.deleteAccessClientId)
            && Self.save(accessClientSecret, with: store.saveAccessClientSecret, orDelete: store.deleteAccessClientSecret)
            && store.saveToken(token)
        if !saved {
            store.deleteToken()
        }
        return saved
    }

    private static func save(
        _ value: String?,
        with save: (String) -> Bool,
        orDelete delete: () -> Void
    ) -> Bool {
        guard let value else {
            delete()
            return true
        }
        return save(value)
    }
}
