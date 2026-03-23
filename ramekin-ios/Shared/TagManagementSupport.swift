import Foundation

extension Notification.Name {
    static let tagsDidChange = Notification.Name("TagsDidChangeNotification")
}

enum TagFilterCache {
    private static let selectedTagsKey = "recipeSelectedTags"
    private static let availableTagsKey = "recipeAvailableTags"

    static func loadSelectedTags() -> Set<String> {
        guard let data = UserDefaults.standard.data(forKey: selectedTagsKey),
              let names = try? JSONDecoder().decode([String].self, from: data) else {
            return []
        }

        return Set(names)
    }

    static func saveSelectedTags(_ selectedTags: Set<String>) {
        guard let data = try? JSONEncoder().encode(Array(selectedTags)) else {
            return
        }

        UserDefaults.standard.set(data, forKey: selectedTagsKey)
    }

    static func pruneSelectedTags(validNames: Set<String>) {
        saveSelectedTags(loadSelectedTags().intersection(validNames))
    }

    static func renameSelectedTag(from oldName: String, to newName: String) {
        var selectedTags = loadSelectedTags()
        guard selectedTags.contains(oldName) else {
            return
        }

        selectedTags.remove(oldName)
        selectedTags.insert(newName)
        saveSelectedTags(selectedTags)
    }

    static func removeSelectedTag(named name: String) {
        var selectedTags = loadSelectedTags()
        guard selectedTags.contains(name) else {
            return
        }

        selectedTags.remove(name)
        saveSelectedTags(selectedTags)
    }

    static func loadAvailableTags() -> [TagItem] {
        guard let data = UserDefaults.standard.data(forKey: availableTagsKey),
              let tags = try? CodableHelper.jsonDecoder.decode([TagItem].self, from: data) else {
            return []
        }

        return tags
    }

    static func saveAvailableTags(_ availableTags: [TagItem]) {
        guard let data = try? CodableHelper.jsonEncoder.encode(availableTags) else {
            return
        }

        UserDefaults.standard.set(data, forKey: availableTagsKey)
    }

    static func notifyTagsDidChange() {
        NotificationCenter.default.post(name: .tagsDidChange, object: nil)
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
                   let errorResponse = try? JSONDecoder().decode(RamekinAPI.ErrorResponse.self, from: data) {
                    return errorResponse.errorMessage
                }

                let description = underlyingError.localizedDescription
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                return description.isEmpty ? fallback : description
            }
        }

        let description = error.localizedDescription.trimmingCharacters(in: .whitespacesAndNewlines)
        return description.isEmpty ? fallback : description
    }
}

enum TagManagementSupport {
    static func normalizedName(from rawName: String) -> String? {
        let trimmed = rawName.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
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

                return TagItem(
                    createdAt: tag.createdAt,
                    id: tag.id,
                    name: newName,
                    recipeCount: tag.recipeCount
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
