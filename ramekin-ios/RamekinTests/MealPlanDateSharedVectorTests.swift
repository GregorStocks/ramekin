import XCTest
@testable import Ramekin

final class MealPlanDateSharedVectorTests: XCTestCase {
    private struct Vector: Decodable {
        let name: String
        let year: Int
        let month: Int
        let day: Int
        let expectedFormatted: String
        let expectedMonday: String
    }

    func testMealPlanDatesMatchSharedVectors() throws {
        let url = try XCTUnwrap(
            Bundle(for: Self.self).url(forResource: "meal-plan-dates", withExtension: "json")
        )
        let vectors = try JSONDecoder().decode([Vector].self, from: Data(contentsOf: url))
        let calendar = localCalendar

        for vector in vectors {
            let date = try XCTUnwrap(calendar.date(from: DateComponents(
                timeZone: calendar.timeZone,
                year: vector.year,
                month: vector.month,
                day: vector.day,
                hour: 12
            )))

            XCTAssertEqual(
                SharedDateFormatters.localDateOnly.string(from: date),
                vector.expectedFormatted,
                vector.name
            )
            XCTAssertEqual(
                SharedDateFormatters.localDateOnly.string(
                    from: MealPlanDateSupport.mondayStart(from: date, calendar: calendar)
                ),
                vector.expectedMonday,
                vector.name
            )
        }
    }

    private var localCalendar: Calendar {
        var calendar = Calendar(identifier: .iso8601)
        calendar.timeZone = SharedDateFormatters.localDateOnly.timeZone
        return calendar
    }
}
