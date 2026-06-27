import Foundation

enum MealPlanDateSupport {
    static func localDate(fromAPIDate apiDate: Date, calendar: Calendar = .current) -> Date {
        let components = apiCalendar.dateComponents([.year, .month, .day], from: apiDate)
        return calendar.date(from: DateComponents(
            calendar: calendar,
            timeZone: calendar.timeZone,
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
