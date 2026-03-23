import SwiftUI

struct RecipeDetailStatusBanner: View {
    enum Style {
        case error
        case progress
    }

    let message: String
    let style: Style

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            if style == .progress {
                ProgressView()
            } else {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundColor(.red)
            }

            Text(message)
                .font(.subheadline)
                .foregroundColor(style == .progress ? .secondary : .primary)
                .multilineTextAlignment(.leading)

            Spacer(minLength: 0)
        }
        .padding(12)
        .background(style == .progress ? Color.orange.opacity(0.12) : Color.red.opacity(0.1))
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }
}

struct PhotoCarouselView: View {
    let photoIds: [UUID]

    var body: some View {
        TabView {
            ForEach(photoIds, id: \.self) { photoId in
                AuthenticatedImage(url: photoURL(for: photoId))
                    .clipped()
            }
        }
        .tabViewStyle(.page)
    }

    private func photoURL(for photoId: UUID) -> URL? {
        guard let baseURL = RamekinAPI.shared.serverURL else { return nil }
        // Use full size for detail view, not thumbnail
        return URL(string: "\(baseURL)/api/photos/\(photoId.uuidString)")
    }
}
