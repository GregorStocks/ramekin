import Foundation

enum SharedDateFormatters {
    // Explicit iso8601 calendar and POSIX locale keep output independent of the
    // user's device calendar (Japanese era, Hebrew, etc.); pinning timeZone to
    // .current ensures the formatted day matches the user's local day. All
    // code that serializes or compares `yyyy-MM-dd` local dates must route
    // through this one formatter so the API payload, UI grouping, and
    // filter-chip parsing agree.
    static let localDateOnly: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        formatter.calendar = Calendar(identifier: .iso8601)
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = .current
        return formatter
    }()
}
