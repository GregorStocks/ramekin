import Foundation

extension RecipeSearchSupport {
    /// The filters over facts only the server knows, which sync carries for
    /// this: source name, stored photo facts and the calorie estimate. A
    /// recipe missing the fact never matches, like SQL NULL.
    static func matchesServerDerivedFilters(
        _ entry: RecipeSearchCorpus.Entry,
        query: PreparedSearchQuery
    ) -> Bool {
        if let source = query.source {
            guard let entrySource = entry.source, bytesContain(entrySource, source) else {
                return false
            }
        }
        guard matchesPhotoThresholds(entry.photos, parsed: query.parsed) else {
            return false
        }
        if let minimum = query.parsed.caloriesMin {
            guard let kcal = entry.kcalPerServing, kcal >= minimum else {
                return false
            }
        }
        if let maximum = query.parsed.caloriesMax {
            guard let kcal = entry.kcalPerServing, kcal <= maximum else {
                return false
            }
        }
        return true
    }

    private static func matchesPhotoThresholds(_ photos: [SyncPhoto], parsed: ParsedSearchQuery) -> Bool {
        if let threshold = parsed.photoSize {
            guard photos.contains(where: { $0.fileSize.map(threshold.matches) ?? false }) else {
                return false
            }
        }
        if let threshold = parsed.photoDim {
            // A photo's smaller dimension, as the server compares.
            let matches = photos.contains { photo in
                guard let width = photo.width, let height = photo.height else { return false }
                return threshold.matches(min(width, height))
            }
            guard matches else {
                return false
            }
        }
        return true
    }
}
