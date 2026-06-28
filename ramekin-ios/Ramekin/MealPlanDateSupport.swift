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

    private static let apiCalendar: Calendar = {
        var calendar = Calendar(identifier: .iso8601)
        calendar.timeZone = TimeZone(secondsFromGMT: 0)!
        return calendar
    }()
}
