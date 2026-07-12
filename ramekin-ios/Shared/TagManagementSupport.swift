import Foundation

extension Notification.Name {
    static let tagsDidChange = Notification.Name("TagsDidChangeNotification")
    static let recipeDeleted = Notification.Name("RecipeDeletedNotification")
}

enum TagFilterCache {
    private static let selectedTagsKeyPrefix = "recipe_selected_tags"
    private static let availableTagsKeyPrefix = "recipe_available_tags"
    private static let legacySelectedTagsKey = "recipeSelectedTags"
    private static let legacyAvailableTagsKey = "recipeAvailableTags"
    private static let legacyMigrationKey = "recipe_tag_cache_account_scope_migrated"

    static func migrateLegacyState(
        activeAccountKey: String?,
        userDefaults: UserDefaults = .standard
    ) {
        guard !userDefaults.bool(forKey: legacyMigrationKey) else { return }

        if let activeAccountKey {
            if let selectedTags = userDefaults.data(forKey: legacySelectedTagsKey) {
                userDefaults.set(selectedTags, forKey: selectedTagsKey(accountKey: activeAccountKey))
            }
            if let availableTags = userDefaults.data(forKey: legacyAvailableTagsKey) {
                userDefaults.set(availableTags, forKey: availableTagsKey(accountKey: activeAccountKey))
            }
        }
        userDefaults.removeObject(forKey: legacySelectedTagsKey)
        userDefaults.removeObject(forKey: legacyAvailableTagsKey)
        userDefaults.set(true, forKey: legacyMigrationKey)
    }

    static func loadSelectedTags(
        accountKey: String,
        userDefaults: UserDefaults = .standard
    ) -> Set<String> {
        guard let data = userDefaults.data(forKey: selectedTagsKey(accountKey: accountKey)),
              let names = try? JSONDecoder().decode([String].self, from: data) else {
            return []
        }

        return Set(names)
    }

    static func saveSelectedTags(
        _ selectedTags: Set<String>,
        accountKey: String,
        userDefaults: UserDefaults = .standard
    ) {
        guard let data = try? JSONEncoder().encode(Array(selectedTags)) else {
            return
        }

        userDefaults.set(data, forKey: selectedTagsKey(accountKey: accountKey))
    }

    static func pruneSelectedTags(
        validNames: Set<String>,
        accountKey: String,
        userDefaults: UserDefaults = .standard
    ) {
        saveSelectedTags(
            loadSelectedTags(accountKey: accountKey, userDefaults: userDefaults).intersection(validNames),
            accountKey: accountKey,
            userDefaults: userDefaults
        )
    }

    static func renameSelectedTag(
        from oldName: String,
        to newName: String,
        accountKey: String,
        userDefaults: UserDefaults = .standard
    ) {
        var selectedTags = loadSelectedTags(accountKey: accountKey, userDefaults: userDefaults)
        guard selectedTags.contains(oldName) else {
            return
        }

        selectedTags.remove(oldName)
        selectedTags.insert(newName)
        saveSelectedTags(selectedTags, accountKey: accountKey, userDefaults: userDefaults)
    }

    static func removeSelectedTag(
        named name: String,
        accountKey: String,
        userDefaults: UserDefaults = .standard
    ) {
        var selectedTags = loadSelectedTags(accountKey: accountKey, userDefaults: userDefaults)
        guard selectedTags.contains(name) else {
            return
        }

        selectedTags.remove(name)
        saveSelectedTags(selectedTags, accountKey: accountKey, userDefaults: userDefaults)
    }

    static func loadAvailableTags(
        accountKey: String,
        userDefaults: UserDefaults = .standard
    ) -> [TagItem] {
        guard let data = userDefaults.data(forKey: availableTagsKey(accountKey: accountKey)),
              let tags = try? CodableHelper.jsonDecoder.decode([TagItem].self, from: data) else {
            return []
        }

        return tags
    }

    static func saveAvailableTags(
        _ availableTags: [TagItem],
        accountKey: String,
        userDefaults: UserDefaults = .standard
    ) {
        guard let data = try? CodableHelper.jsonEncoder.encode(availableTags) else {
            return
        }

        userDefaults.set(data, forKey: availableTagsKey(accountKey: accountKey))
    }

    static func notifyTagsDidChange() {
        NotificationCenter.default.post(name: .tagsDidChange, object: nil)
    }

    private static func selectedTagsKey(accountKey: String) -> String {
        AccountScope.userDefaultsKey(prefix: selectedTagsKeyPrefix, accountKey: accountKey)
    }

    private static func availableTagsKey(accountKey: String) -> String {
        AccountScope.userDefaultsKey(prefix: availableTagsKeyPrefix, accountKey: accountKey)
    }
}

enum APIErrorFormatter {
    static func userMessage(from error: Error, fallback: String) -> String {
        if let apiError = error as? RamekinAPI.APIError,
           let description = apiError.errorDescription,
           !description.isEmpty {
            return description
        }

        if let generatedError = error as? ErrorResponse {
            switch generatedError {
            case .error(_, let data, _, let underlyingError):
                if let data,
                   let errorResponse = try? JSONDecoder().decode(ModelErrorResponse.self, from: data) {
                    return errorResponse.error
                }

                let description = underlyingError.localizedDescription
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                return description.isEmpty ? fallback : description
            }
        }

        let description = error.localizedDescription.trimmingCharacters(in: .whitespacesAndNewlines)
        return description.isEmpty ? fallback : description
    }

    /// Extract the server's machine-readable error code from any API error,
    /// whether it came through `RamekinAPI.APIError` or the generated client's
    /// `ErrorResponse`. Branch on this rather than the HTTP status or message.
    static func code(from error: Error) -> ErrorCode? {
        if let apiError = error as? RamekinAPI.APIError {
            return apiError.code
        }

        if let generatedError = error as? ErrorResponse {
            switch generatedError {
            case .error(_, let data, _, _):
                if let data,
                   let errorResponse = try? JSONDecoder().decode(ModelErrorResponse.self, from: data) {
                    return errorResponse.code
                }
            }
        }

        return nil
    }
}

enum TagManagementSupport {
    static func parseTagName(_ name: String) -> (namespace: String?, value: String) {
        let parsed = TagHierarchySupport.parse(name: name)
        return (namespace: parsed.namespace, value: parsed.value)
    }

    static func normalizedName(from rawName: String) -> String? {
        TagHierarchySupport.normalizedValue(from: rawName)
    }

    static func recipeCountText(for count: Int64) -> String {
        "\(count) \(count == 1 ? "recipe" : "recipes")"
    }

    static func renamedTags(_ tags: [TagItem], id: UUID, newName: String) -> [TagItem] {
        tags
            .map { tag in
                guard tag.id == id else {
                    return tag
                }

                let parsed = parseTagName(newName)
                return TagItem(
                    createdAt: tag.createdAt,
                    id: tag.id,
                    name: newName,
                    namespace: parsed.namespace,
                    recipeCount: tag.recipeCount,
                    value: parsed.value
                )
            }
            .sorted {
                $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending
            }
    }

    static func removingTag(_ tags: [TagItem], id: UUID) -> [TagItem] {
        tags.filter { $0.id != id }
    }
}
