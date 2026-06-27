import Foundation

struct RecipeListFilterState: Equatable {
    var searchText: String = ""
    var selectedTags: Set<String> = []
    var photoFilter: PhotoFilter = .any
    var source: String = ""
    var createdAfter: String = ""
    var createdBefore: String = ""
    var photoSizeThreshold: NumericThreshold?
    var photoDimensionThreshold: NumericThreshold?

    var hasAdvancedFilters: Bool {
        !RecipeListFilterSupport.normalizedSource(source).isEmpty
            || !RecipeListFilterSupport.normalizedDateValue(createdAfter).isEmpty
            || !RecipeListFilterSupport.normalizedDateValue(createdBefore).isEmpty
            || photoSizeThreshold != nil
            || photoDimensionThreshold != nil
    }

    var hasAnyFilters: Bool {
        !selectedTags.isEmpty || photoFilter != .any || hasAdvancedFilters
    }
}

enum NumericThresholdOperator: String, CaseIterable {
    case lessThan = "<"
    case greaterThan = ">"

    var label: String {
        switch self {
        case .lessThan:
            return "<"
        case .greaterThan:
            return ">"
        }
    }
}

struct NumericThreshold: Equatable {
    let comparison: NumericThresholdOperator
    let value: Int

    var queryValue: String {
        "\(comparison.rawValue)\(value)"
    }
}

enum RecipeListFilterSupport {
    static func buildQuery(from state: RecipeListFilterState) -> String? {
        var parts: [String] = []

        let trimmedSearchText = state.searchText.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmedSearchText.isEmpty {
            parts.append(trimmedSearchText)
        }

        for tag in state.selectedTags.sorted() {
            parts.append(filterToken(prefix: "tag", value: tag))
        }

        let normalizedSource = normalizedSource(state.source)
        if !normalizedSource.isEmpty {
            parts.append(filterToken(prefix: "source", value: normalizedSource))
        }

        if let photoFilterToken = photoFilterToken(for: state.photoFilter) {
            parts.append(photoFilterToken)
        }

        if let photoSizeThreshold = state.photoSizeThreshold {
            parts.append("photo_size:\(photoSizeThreshold.queryValue)")
        }

        if let photoDimensionThreshold = state.photoDimensionThreshold {
            parts.append("photo_dim:\(photoDimensionThreshold.queryValue)")
        }

        if let createdDateToken = createdDateToken(
            createdAfter: state.createdAfter,
            createdBefore: state.createdBefore
        ) {
            parts.append(createdDateToken)
        }

        return parts.isEmpty ? nil : parts.joined(separator: " ")
    }

    static func advancedFilterLabel(for state: RecipeListFilterState) -> String? {
        let normalizedSource = normalizedSource(state.source)
        let createdLabel = createdFilterLabel(
            createdAfter: state.createdAfter,
            createdBefore: state.createdBefore
        )
        let hasPhotoThreshold = state.photoSizeThreshold != nil || state.photoDimensionThreshold != nil

        switch (normalizedSource.isEmpty, createdLabel == nil, hasPhotoThreshold) {
        case (true, true, false):
            return nil
        case (false, true, false):
            return normalizedSource
        case (true, false, false):
            return createdLabel
        default:
            return "Advanced"
        }
    }

    static func normalizedSource(_ source: String) -> String {
        source.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    static func normalizedDateValue(_ rawValue: String) -> String {
        rawValue.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    static func dateOnlyString(from date: Date) -> String {
        SharedDateFormatters.localDateOnly.string(from: date)
    }

    static func date(from rawValue: String) -> Date? {
        let normalizedValue = normalizedDateValue(rawValue)
        guard !normalizedValue.isEmpty else {
            return nil
        }

        return SharedDateFormatters.localDateOnly.date(from: normalizedValue)
    }

    static func numericThreshold(from rawValue: String) -> NumericThreshold? {
        let normalizedValue = rawValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let opCharacter = normalizedValue.first,
              let comparison = NumericThresholdOperator(rawValue: String(opCharacter)) else {
            return nil
        }

        let valueText = normalizedValue.dropFirst().trimmingCharacters(in: .whitespacesAndNewlines)
        guard let value = Int(valueText) else {
            return nil
        }

        return NumericThreshold(comparison: comparison, value: value)
    }

    static func thresholdQueryValue(
        isEnabled: Bool,
        comparison: NumericThresholdOperator,
        value: String
    ) -> String {
        guard isEnabled else {
            return ""
        }

        let normalizedValue = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let intValue = Int(normalizedValue) else {
            return ""
        }

        return NumericThreshold(comparison: comparison, value: intValue).queryValue
    }

    private static func createdFilterLabel(createdAfter: String, createdBefore: String) -> String? {
        let afterLabel = dateChipLabel(from: createdAfter)
        let beforeLabel = dateChipLabel(from: createdBefore)

        switch (afterLabel, beforeLabel) {
        case (nil, nil):
            return nil
        case let (after?, before?) where after == before:
            return after
        case let (after?, before?):
            return "\(after)-\(before)"
        case let (after?, nil):
            return "After \(after)"
        case let (nil, before?):
            return "Before \(before)"
        }
    }

    private static func dateChipLabel(from rawValue: String) -> String? {
        guard let date = date(from: rawValue) else {
            return nil
        }

        return chipDateFormatter.string(from: date)
    }

    private static func photoFilterToken(for photoFilter: PhotoFilter) -> String? {
        switch photoFilter {
        case .any:
            return nil
        case .hasPhotos:
            return "has:photos"
        case .noPhotos:
            return "no:photos"
        }
    }

    private static func createdDateToken(createdAfter: String, createdBefore: String) -> String? {
        let normalizedCreatedAfter = normalizedDateValue(createdAfter)
        let normalizedCreatedBefore = normalizedDateValue(createdBefore)

        if !normalizedCreatedAfter.isEmpty && !normalizedCreatedBefore.isEmpty {
            if normalizedCreatedAfter == normalizedCreatedBefore {
                return "created:\(normalizedCreatedAfter)"
            }

            return "created:\(normalizedCreatedAfter)..\(normalizedCreatedBefore)"
        }

        if !normalizedCreatedAfter.isEmpty {
            return "created:>\(normalizedCreatedAfter)"
        }

        if !normalizedCreatedBefore.isEmpty {
            return "created:<\(normalizedCreatedBefore)"
        }

        return nil
    }

    private static func filterToken(prefix: String, value: String) -> String {
        if value.contains(" ") || value.contains(":") {
            return "\(prefix):\"\(value)\""
        }

        return "\(prefix):\(value)"
    }

    // Pair timeZone with SharedDateFormatters.localDateOnly (.autoupdatingCurrent)
    // so parsing a chip's yyyy-MM-dd string and then displaying it in "MMM d"
    // can't disagree on the calendar day after a runtime timezone change.
    private static let chipDateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateFormat = "MMM d"
        formatter.calendar = Calendar(identifier: .iso8601)
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = .autoupdatingCurrent
        return formatter
    }()
}
