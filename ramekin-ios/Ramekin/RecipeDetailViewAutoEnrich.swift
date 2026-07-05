import SwiftUI

extension RecipeDetailView {
    func autoEnrichmentProgressBanner(_ label: String) -> some View {
        HStack(spacing: 10) {
            ProgressView()
            Text(label)
                .font(.subheadline)
                .foregroundColor(.primary)
            Spacer()
        }
        .padding(12)
        .background(Color.purple.opacity(0.14))
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }
}
