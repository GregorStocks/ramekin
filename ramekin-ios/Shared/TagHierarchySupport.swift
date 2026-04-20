import Foundation

enum TagHierarchySupport {
    static let seededNamespaces = [
        "ingredient",
        "course",
        "cuisine",
        "diet",
        "method",
        "season"
    ]

    static let uncategorizedTitle = "Uncategorized"

    struct ParsedTag: Equatable {
        let name: String
        let namespace: String?
        let value: String
    }

    struct TagGroup<Item>: Identifiable {
        let namespace: String?
        let title: String
        let items: [Item]

        var id: String {
            namespace ?? "__uncategorized__"
        }
    }

    static func parse(name: String) -> ParsedTag {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let parts = trimmed.split(separator: ":", maxSplits: 1, omittingEmptySubsequences: false)
        if parts.count == 2 {
            let namespace = parts[0].trimmingCharacters(in: .whitespacesAndNewlines)
            let value = parts[1].trimmingCharacters(in: .whitespacesAndNewlines)
            if !namespace.isEmpty && !value.isEmpty {
                return ParsedTag(
                    name: trimmed,
                    namespace: namespace,
                    value: value
                )
            }
        }

        return ParsedTag(name: trimmed, namespace: nil, value: trimmed)
    }

    static func normalizedValue(from rawValue: String) -> String? {
        let trimmed = rawValue.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    static func normalizedNamespace(from rawNamespace: String) -> String? {
        let trimmed = rawNamespace
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
        guard !trimmed.isEmpty else {
            return nil
        }
        guard isValidNamespace(trimmed) else {
            return nil
        }
        return trimmed
    }

    static func formattedName(namespace: String?, value: String) -> String? {
        guard let normalizedValue = normalizedValue(from: value) else {
            return nil
        }
        guard let namespace else {
            return normalizedValue
        }
        guard let normalizedNamespace = normalizedNamespace(from: namespace) else {
            return nil
        }
        return "\(normalizedNamespace):\(normalizedValue)"
    }

    static func availableNamespaces(from names: [String]) -> [String] {
        var discovered = Set<String>()
        for name in names {
            let parsed = parse(name: name)
            if let namespace = parsed.namespace {
                discovered.insert(namespace)
            }
        }

        return orderedNamespaces(from: discovered)
    }

    static func groups(for tags: [TagItem]) -> [TagGroup<TagItem>] {
        let grouped = Dictionary(grouping: tags) { tag in
            parsedTag(for: tag).namespace
        }

        return orderedGroups(grouped) { lhs, rhs in
            parsedTag(for: lhs).value.localizedCaseInsensitiveCompare(parsedTag(for: rhs).value) == .orderedAscending
        }
    }

    static func groups(for names: [String]) -> [TagGroup<ParsedTag>] {
        let parsedNames = names.map(parse(name:))
        let grouped = Dictionary(grouping: parsedNames, by: \.namespace)

        return orderedGroups(grouped) { lhs, rhs in
            lhs.value.localizedCaseInsensitiveCompare(rhs.value) == .orderedAscending
        }
    }

    static func title(for namespace: String?) -> String {
        namespace ?? uncategorizedTitle
    }

    private static func parsedTag(for tag: TagItem) -> ParsedTag {
        let parsed = parse(name: tag.name)
        return ParsedTag(
            name: tag.name,
            namespace: tag.namespace ?? parsed.namespace,
            value: tag.value.isEmpty ? parsed.value : tag.value
        )
    }

    private static func orderedNamespaces(from namespaces: Set<String>) -> [String] {
        let extras = namespaces
            .filter { !seededNamespaces.contains($0) }
            .sorted { $0.localizedCaseInsensitiveCompare($1) == .orderedAscending }

        return seededNamespaces.filter { namespaces.contains($0) } + extras
    }

    private static func orderedGroups<Item>(
        _ grouped: [String?: [Item]],
        sorter: (Item, Item) -> Bool
    ) -> [TagGroup<Item>] {
        let namespaces = Set(grouped.keys.compactMap { $0 })
        var groups: [TagGroup<Item>] = orderedNamespaces(from: namespaces).compactMap { namespace in
            guard let items = grouped[namespace], !items.isEmpty else {
                return nil
            }
            return TagGroup(
                namespace: namespace,
                title: title(for: namespace),
                items: items.sorted(by: sorter)
            )
        }

        if let uncategorized = grouped[nil], !uncategorized.isEmpty {
            groups.append(
                TagGroup(
                    namespace: nil,
                    title: title(for: nil),
                    items: uncategorized.sorted(by: sorter)
                )
            )
        }

        return groups
    }

    private static func isValidNamespace(_ namespace: String) -> Bool {
        guard let first = namespace.first, first.isLetter, first.isLowercase else {
            return false
        }

        for character in namespace {
            guard character.isLetter || character.isNumber || character == "-" || character == "_" else {
                return false
            }
            if character.isUppercase {
                return false
            }
        }

        return true
    }
}
