import SwiftUI

extension RecipeDetailView {
    func rescrapeProgressBanner() -> some View {
        HStack(spacing: 10) {
            ProgressView()
            Text("Rescraping from source...")
                .font(.subheadline)
                .foregroundColor(.primary)
            Spacer()
        }
        .padding(12)
        .background(Color.orange.opacity(0.14))
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }
}
