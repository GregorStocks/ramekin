import Foundation
import Security

/// Helper for storing and retrieving credentials from the iOS Keychain
/// Uses an App Group so the main app and Share Extension can share data
class KeychainHelper {
    static let shared = KeychainHelper()

    // IMPORTANT: Change this to match your App Group identifier
    private let accessGroup = "group.com.ramekin.app"
    private let service = "com.ramekin.app"

    private init() {}

    // MARK: - Auth Token

    private let tokenKey = "authToken"

    func saveToken(_ token: String) -> Bool {
        return save(key: tokenKey, data: Data(token.utf8))
    }

    func getToken() -> String? {
        guard let data = load(key: tokenKey) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    func deleteToken() {
        delete(key: tokenKey)
    }

    // MARK: - Server URL

    private let serverURLKey = "serverURL"

    func saveServerURL(_ url: String) -> Bool {
        return save(key: serverURLKey, data: Data(url.utf8))
    }

    func getServerURL() -> String? {
        guard let data = load(key: serverURLKey) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    // MARK: - Username (for display only)

    private let usernameKey = "username"

    func saveUsername(_ username: String) -> Bool {
        return save(key: usernameKey, data: Data(username.utf8))
    }

    func getUsername() -> String? {
        guard let data = load(key: usernameKey) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    // MARK: - Private Keychain Operations

    private func save(key: String, data: Data) -> Bool {
        // Delete existing item first
        delete(key: key)

        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecValueData as String: data,
            kSecAttrAccessGroup as String: accessGroup,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock
        ]

        let status = SecItemAdd(query as CFDictionary, nil)
        return status == errSecSuccess
    }

    private func load(key: String) -> Data? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecAttrAccessGroup as String: accessGroup,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne
        ]

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)

        guard status == errSecSuccess else { return nil }
        return result as? Data
    }

    private func delete(key: String) {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecAttrAccessGroup as String: accessGroup
        ]

        SecItemDelete(query as CFDictionary)
    }

    /// Clear all stored credentials (for logout)
    func clearAll() {
        deleteToken()
        delete(key: serverURLKey)
        delete(key: usernameKey)
    }
}
