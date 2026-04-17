import XCTest
@testable import Ramekin

final class DateFormatterCachingTests: XCTestCase {
    func testMealPlanFormattersAreCachedWithExpectedConfiguration() {
        XCTAssertTrue(MealPlanView.weekRangeFormatter === MealPlanView.weekRangeFormatter)
        XCTAssertEqual(MealPlanView.weekRangeFormatter.dateFormat, "MMM d")

        XCTAssertTrue(MealPlanView.dayHeaderFormatter === MealPlanView.dayHeaderFormatter)
        XCTAssertEqual(MealPlanView.dayHeaderFormatter.dateFormat, "EEEE, MMM d")
    }

    func testSharedLocalDateFormatterIsCachedWithExpectedConfiguration() {
        XCTAssertTrue(SharedDateFormatters.localDateOnly === SharedDateFormatters.localDateOnly)
        XCTAssertEqual(SharedDateFormatters.localDateOnly.dateFormat, "yyyy-MM-dd")
        XCTAssertEqual(SharedDateFormatters.localDateOnly.locale.identifier, "en_US_POSIX")
        XCTAssertEqual(SharedDateFormatters.localDateOnly.calendar.identifier, .iso8601)
    }

    func testSharedLocalDateFormatterRoundTripsISODate() {
        let input = "2026-04-17"
        let parsed = SharedDateFormatters.localDateOnly.date(from: input)
        XCTAssertNotNil(parsed)
        XCTAssertEqual(SharedDateFormatters.localDateOnly.string(from: parsed!), input)
    }

    func testRecipeDetailVersionFormatterIsCachedWithExpectedStyles() {
        XCTAssertTrue(
            RecipeDetailView.versionHistoryDateFormatter === RecipeDetailView.versionHistoryDateFormatter
        )
        XCTAssertEqual(RecipeDetailView.versionHistoryDateFormatter.dateStyle, .medium)
        XCTAssertEqual(RecipeDetailView.versionHistoryDateFormatter.timeStyle, .short)
    }
}
