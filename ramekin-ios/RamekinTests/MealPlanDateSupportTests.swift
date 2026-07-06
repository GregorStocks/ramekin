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

    func testMondayStartReturnsSameMondayAtStartOfDay() {
        let date = utcCalendar.date(from: DateComponents(
            timeZone: TimeZone(secondsFromGMT: 0)!,
            year: 2026,
            month: 7,
            day: 6,
            hour: 14,
            minute: 30
        ))!

        let monday = MealPlanDateSupport.mondayStart(from: date, calendar: utcCalendar)
        let components = utcCalendar.dateComponents([.year, .month, .day, .hour, .minute], from: monday)

        XCTAssertEqual(components.year, 2026)
        XCTAssertEqual(components.month, 7)
        XCTAssertEqual(components.day, 6)
        XCTAssertEqual(components.hour, 0)
        XCTAssertEqual(components.minute, 0)
    }

    func testMondayStartBacksUpFromSunday() {
        let date = utcCalendar.date(from: DateComponents(
            timeZone: TimeZone(secondsFromGMT: 0)!,
            year: 2026,
            month: 7,
            day: 12,
            hour: 8
        ))!

        let monday = MealPlanDateSupport.mondayStart(from: date, calendar: utcCalendar)
        let components = utcCalendar.dateComponents([.year, .month, .day], from: monday)

        XCTAssertEqual(components.year, 2026)
        XCTAssertEqual(components.month, 7)
        XCTAssertEqual(components.day, 6)
    }

    private let utcCalendar: Calendar = {
        var calendar = Calendar(identifier: .iso8601)
        calendar.timeZone = TimeZone(secondsFromGMT: 0)!
        return calendar
    }()
}
