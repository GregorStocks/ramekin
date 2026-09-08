import UIKit
import XCTest

final class RecipeActionColorTests: XCTestCase {
    func testRecipeAccentAndSelectedTextContrastInBothAppearances() throws {
        let accent = try XCTUnwrap(UIColor(named: "AccentColor"))
        for appearance in [UIUserInterfaceStyle.light, .dark] {
            let traits = UITraitCollection(userInterfaceStyle: appearance)
            let accentLuminance = luminance(accent.resolvedColor(with: traits))
            let backgroundLuminance = luminance(UIColor.systemBackground.resolvedColor(with: traits))
            // The same pair is used for accent text on the page and for
            // selected controls with the foreground/background inverted.
            let contrast = (max(accentLuminance, backgroundLuminance) + 0.05)
                / (min(accentLuminance, backgroundLuminance) + 0.05)
            XCTAssertGreaterThanOrEqual(contrast, 4.5, "Insufficient contrast in \(appearance)")
        }
    }

    private func luminance(_ color: UIColor) -> Double {
        var red: CGFloat = 0
        var green: CGFloat = 0
        var blue: CGFloat = 0
        var alpha: CGFloat = 0
        XCTAssertTrue(color.getRed(&red, green: &green, blue: &blue, alpha: &alpha))
        XCTAssertEqual(alpha, 1)
        let channels = [red, green, blue].map { component -> Double in
            let value = Double(component)
            return value <= 0.04045 ? value / 12.92 : pow((value + 0.055) / 1.055, 2.4)
        }
        return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722
    }
}
