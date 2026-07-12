import Foundation

enum AccountScope {
    static func key(serverURL: String, username: String) -> String {
        "\(serverURL)|\(username)"
    }

    static func currentAccountKey(credentialStore: CredentialStore = KeychainHelper.shared) -> String? {
        guard let serverURL = credentialStore.getServerURL(),
              let username = credentialStore.getUsername()
        else {
            return nil
        }
        return key(serverURL: serverURL, username: username)
    }

    static func userDefaultsKey(prefix: String, accountKey: String) -> String {
        let encodedAccountKey = Data(accountKey.utf8).base64EncodedString()
        return "\(prefix)_\(encodedAccountKey)"
    }
}
