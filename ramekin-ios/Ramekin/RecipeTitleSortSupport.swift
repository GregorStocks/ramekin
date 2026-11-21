import Foundation

enum RecipeTitleSortSupport {
    static func areInIncreasingOrder(
        lhsTitle: String,
        lhsID: UUID,
        rhsTitle: String,
        rhsID: UUID,
        descending: Bool = false
    ) -> Bool {
        areInIncreasingOrder(
            lhsKey: sortKey(lhsTitle),
            lhsID: lhsID.uuidString,
            rhsKey: sortKey(rhsTitle),
            rhsID: rhsID.uuidString,
            descending: descending
        )
    }

    /// The same ordering over keys from `sortKey` and `uuidString` ids, so
    /// sorting a whole list folds each title once instead of once per
    /// comparison.
    static func areInIncreasingOrder(
        lhsKey: [UInt32],
        lhsID: String,
        rhsKey: [UInt32],
        rhsID: String,
        descending: Bool = false
    ) -> Bool {
        let titleComparison = compareKeys(descending ? rhsKey : lhsKey, descending ? lhsKey : rhsKey)
        if titleComparison == .orderedSame {
            return lhsID < rhsID
        }
        return titleComparison == .orderedAscending
    }

    private static func compareKeys(_ lhsScalars: [UInt32], _ rhsScalars: [UInt32]) -> ComparisonResult {
        for (lhsScalar, rhsScalar) in zip(lhsScalars, rhsScalars) where lhsScalar != rhsScalar {
            return lhsScalar < rhsScalar ? .orderedAscending : .orderedDescending
        }
        if lhsScalars.count == rhsScalars.count {
            return .orderedSame
        }
        return lhsScalars.count < rhsScalars.count ? .orderedAscending : .orderedDescending
    }

    /// A title folded for comparison.
    static func sortKey(_ title: String) -> [UInt32] {
        title.unicodeScalars.flatMap { scalar in
            String(scalar)
                .lowercased(with: foldingLocale)
                .unicodeScalars
                .map(\.value)
        }
    }

    private static let foldingLocale = Locale(identifier: "en_US_POSIX")
}
