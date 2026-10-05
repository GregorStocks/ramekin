import SwiftUI

extension RecipeDetailView {
    func progressBanner(_ message: String) -> some View {
        HStack(spacing: 10) {
            ProgressView()
            Text(message)
                .font(.subheadline)
                .foregroundColor(.primary)
            Spacer()
        }
        .padding(12)
        .background(Color.orange.opacity(0.14))
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }
}
