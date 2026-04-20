import SwiftUI

struct HierarchicalTagLabel: View {
    let name: String
    var valueFont: Font
    var namespaceFont: Font
    var valueColor: Color
    var namespaceColor: Color

    init(
        name: String,
        valueFont: Font = .caption,
        namespaceFont: Font = .caption2,
        valueColor: Color = .primary,
        namespaceColor: Color = .secondary
    ) {
        self.name = name
        self.valueFont = valueFont
        self.namespaceFont = namespaceFont
        self.valueColor = valueColor
        self.namespaceColor = namespaceColor
    }

    var body: some View {
        label
            .lineLimit(1)
    }

    private var label: Text {
        let parsed = TagHierarchySupport.parse(name: name)
        guard let namespace = parsed.namespace else {
            return Text(parsed.value)
                .font(valueFont)
                .foregroundColor(valueColor)
        }

        return Text("\(namespace):")
            .font(namespaceFont)
            .foregroundColor(namespaceColor)
        + Text(parsed.value)
            .font(valueFont)
            .foregroundColor(valueColor)
    }
}

struct HierarchicalTagChip: View {
    let name: String
    var isSelected: Bool = false
    var baseColor: Color = .orange
    var valueFont: Font = .caption
    var namespaceFont: Font = .caption2

    var body: some View {
        HierarchicalTagLabel(
            name: name,
            valueFont: valueFont,
            namespaceFont: namespaceFont,
            valueColor: isSelected ? .white : baseColor,
            namespaceColor: isSelected ? Color.white.opacity(0.78) : .secondary
        )
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(backgroundColor)
        .clipShape(Capsule())
    }

    private var backgroundColor: Color {
        isSelected ? baseColor : baseColor.opacity(0.16)
    }
}
