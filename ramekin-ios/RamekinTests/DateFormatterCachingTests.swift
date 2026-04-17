import XCTest
@testable import Ramekin

final class DateFormatterCachingTests: XCTestCase {
    func testMealPlanFormattersAreCachedWithExpectedConfiguration() {
        XCTAssertTrue(MealPlanView.weekRangeFormatter === MealPlanView.weekRangeFormatter)
        XCTAssertEqual(MealPlanView.weekRangeFormatter.dateFormat, "MMM d")

        XCTAssertTrue(MealPlanView.dayHeaderFormatter === MealPlanView.dayHeaderFormatter)
        XCTAssertEqual(MealPlanView.dayHeaderFormatter.dateFormat, "EEEE, MMM d")

        XCTAssertTrue(MealPlanView.localDateFormatter === MealPlanView.localDateFormatter)
        XCTAssertEqual(MealPlanView.localDateFormatter.dateFormat, "yyyy-MM-dd")
    }

    func testRecipeDetailVersionFormatterIsCachedWithExpectedStyles() {
        XCTAssertTrue(
            RecipeDetailView.versionHistoryDateFormatter === RecipeDetailView.versionHistoryDateFormatter
        )
        XCTAssertEqual(RecipeDetailView.versionHistoryDateFormatter.dateStyle, .medium)
        XCTAssertEqual(RecipeDetailView.versionHistoryDateFormatter.timeStyle, .short)
    }
}
