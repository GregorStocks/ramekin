import Foundation

enum RecipeTitleSortSupport {
    static func areInIncreasingOrder(
        lhsTitle: String,
        lhsID: UUID,
        rhsTitle: String,
        rhsID: UUID,
        descending: Bool = false
    ) -> Bool {
        let titleComparison = compareFoldedTitles(
            descending ? rhsTitle : lhsTitle,
            descending ? lhsTitle : rhsTitle
        )
        if titleComparison == .orderedSame {
            return lhsID.uuidString < rhsID.uuidString
        }
        return titleComparison == .orderedAscending
    }

    private static func compareFoldedTitles(_ lhs: String, _ rhs: String) -> ComparisonResult {
        let lhsScalars = foldedScalars(lhs)
        let rhsScalars = foldedScalars(rhs)

        for (lhsScalar, rhsScalar) in zip(lhsScalars, rhsScalars) where lhsScalar != rhsScalar {
            return lhsScalar < rhsScalar ? .orderedAscending : .orderedDescending
        }
        if lhsScalars.count == rhsScalars.count {
            return .orderedSame
        }
        return lhsScalars.count < rhsScalars.count ? .orderedAscending : .orderedDescending
    }

    private static func foldedScalars(_ title: String) -> [UInt32] {
        title.unicodeScalars.flatMap { scalar in
            String(scalar)
                .lowercased(with: foldingLocale)
                .unicodeScalars
                .map(\.value)
        }
    }

    private static let foldingLocale = Locale(identifier: "en_US_POSIX")
}
