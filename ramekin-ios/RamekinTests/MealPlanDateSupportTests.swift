import XCTest
@testable import Ramekin

final class MealPlanDateSupportTests: XCTestCase {
    func testLocalDatePreservesAPIDateInWesternTimeZone() {
        let pacificTime = TimeZone(identifier: "America/Los_Angeles")!

        let apiDate = utcCalendar.date(from: DateComponents(
            timeZone: TimeZone(secondsFromGMT: 0)!,
            year: 2026,
            month: 4,
            day: 18
        ))!

        let localDate = MealPlanDateSupport.localDate(fromAPIDate: apiDate, timeZone: pacificTime)
        var localCalendar = Calendar(identifier: .iso8601)
        localCalendar.timeZone = pacificTime
        let components = localCalendar.dateComponents([.year, .month, .day], from: localDate)

        XCTAssertEqual(components.year, 2026)
        XCTAssertEqual(components.month, 4)
        XCTAssertEqual(components.day, 18)
    }

    private let utcCalendar: Calendar = {
        var calendar = Calendar(identifier: .iso8601)
        calendar.timeZone = TimeZone(secondsFromGMT: 0)!
        return calendar
    }()
}
