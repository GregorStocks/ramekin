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
        SharedDateFormatters.localDateOnly.string(from: localDate(fromAPIDate: apiDate))
    }

    /// The date the generated client would decode for an API day.
    static func apiDate(fromDay day: String) -> Date? {
        guard let local = SharedDateFormatters.localDateOnly.date(from: day) else { return nil }
        let components = SharedDateFormatters.localDateOnly.calendar.dateComponents(
            in: SharedDateFormatters.localDateOnly.timeZone,
            from: local
        )
        return apiCalendar.date(from: DateComponents(
            year: components.year,
            month: components.month,
            day: components.day
        ))
    }

    private static let apiCalendar: Calendar = {
        var calendar = Calendar(identifier: .iso8601)
        calendar.timeZone = TimeZone(secondsFromGMT: 0)!
        return calendar
    }()
}
