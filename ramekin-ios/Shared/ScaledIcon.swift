import SwiftUI

/// A decorative SF Symbol at a fixed design size that still grows and shrinks
/// with Dynamic Type, unlike `.font(.system(size:))`.
private struct ScaledIconModifier: ViewModifier {
    @ScaledMetric private var size: CGFloat

    init(size: CGFloat) {
        _size = ScaledMetric(wrappedValue: size, relativeTo: .largeTitle)
    }

    func body(content: Content) -> some View {
        content.font(.system(size: size))
    }
}

extension View {
    func scaledIconFont(size: CGFloat) -> some View {
        modifier(ScaledIconModifier(size: size))
    }
}
