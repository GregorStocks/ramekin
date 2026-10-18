import Foundation

/// A sync feed's cursor, stored per account in UserDefaults.
struct SyncCursorStore {
    let prefix: String

    func cursor(accountKey: String, in defaults: UserDefaults) -> Int64? {
        defaults.object(forKey: key(accountKey)) as? Int64
    }

    func set(_ cursor: Int64, accountKey: String, in defaults: UserDefaults) {
        defaults.set(cursor, forKey: key(accountKey))
    }

    private func key(_ accountKey: String) -> String {
        AccountScope.userDefaultsKey(prefix: prefix, accountKey: accountKey)
    }
}
