import XCTest
@testable import Ramekin

final class MealPlanDateSupportTests: XCTestCase {
    func testLocalDatePreservesAPIDateInWesternTimeZone() {
        let originalTimeZone = NSTimeZone.default
        NSTimeZone.default = TimeZone(identifier: "America/Los_Angeles")!
        defer { NSTimeZone.default = originalTimeZone }

        let apiDate = utcCalendar.date(from: DateComponents(
            timeZone: TimeZone(secondsFromGMT: 0)!,
            year: 2026,
            month: 4,
            day: 18
        ))!

        let localDate = MealPlanDateSupport.localDate(fromAPIDate: apiDate)

        XCTAssertEqual(SharedDateFormatters.localDateOnly.string(from: localDate), "2026-04-18")
    }

    private let utcCalendar: Calendar = {
        var calendar = Calendar(identifier: .iso8601)
        calendar.timeZone = TimeZone(secondsFromGMT: 0)!
        return calendar
    }()
}
