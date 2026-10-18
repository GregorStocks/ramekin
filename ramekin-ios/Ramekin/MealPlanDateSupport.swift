import Foundation

enum MealPlanDateSupport {
    static func localDate(fromAPIDate apiDate: Date, timeZone: TimeZone = .autoupdatingCurrent) -> Date {
        let components = apiCalendar.dateComponents([.year, .month, .day], from: apiDate)
        var localCalendar = Calendar(identifier: .iso8601)
        localCalendar.timeZone = timeZone
        return localCalendar.date(from: DateComponents(
            calendar: localCalendar,
            timeZone: timeZone,
            year: components.year,
            month: components.month,
            day: components.day
        ))!
    }

    static func mondayStart(from date: Date, calendar: Calendar = .current) -> Date {
        let weekday = calendar.component(.weekday, from: date)
        let daysToSubtract = (weekday + 5) % 7
        guard let monday = calendar.date(
            byAdding: .day,
            value: -daysToSubtract,
            to: calendar.startOfDay(for: date)
        ) else {
            preconditionFailure("Unable to calculate Monday week start")
        }
        return monday
    }

    /// The API's `yyyy-MM-dd` day for a date the generated client decoded,
    /// which it reads as UTC midnight.
    static func apiDay(fromAPIDate apiDate: Date) -> String {
        apiDayFormatter.string(from: apiDate)
    }

    /// The date the generated client would decode for an API day.
    static func apiDate(fromDay day: String) -> Date? {
        apiDayFormatter.date(from: day)
    }

    private static let apiDayFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        formatter.calendar = Calendar(identifier: .iso8601)
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = TimeZone(secondsFromGMT: 0)
        return formatter
    }()

    private static let apiCalendar: Calendar = {
        var calendar = Calendar(identifier: .iso8601)
        calendar.timeZone = TimeZone(secondsFromGMT: 0)!
        return calendar
    }()
}
