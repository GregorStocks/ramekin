import SwiftUI

/// Loads images from authenticated endpoints with Bearer token
@MainActor
class AuthenticatedImageLoader: ObservableObject {
    @Published var image: UIImage?
    @Published var isLoading = false
    @Published var error: Error?

    private var currentTask: Task<Void, Never>?

    func load(url: URL) {
        // Cancel any existing load
        currentTask?.cancel()

        guard let token = RamekinAPI.shared.authToken else {
            return
        }

        isLoading = true
        error = nil

        currentTask = Task {
            var request = URLRequest(url: url)
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")

            do {
                let (data, response) = try await URLSession.shared.data(for: request)

                guard !Task.isCancelled else { return }

                guard let httpResponse = response as? HTTPURLResponse,
                      httpResponse.statusCode == 200 else {
                    throw URLError(.badServerResponse)
                }

                if let loadedImage = UIImage(data: data) {
                    self.image = loadedImage
                }
            } catch {
                guard !Task.isCancelled else { return }
                self.error = error
            }

            self.isLoading = false
        }
    }

    func cancel() {
        currentTask?.cancel()
        currentTask = nil
    }
}

/// SwiftUI view for displaying authenticated images with loading/error states
struct AuthenticatedImage: View {
    let url: URL?
    let contentMode: ContentMode

    @StateObject private var loader = AuthenticatedImageLoader()

    init(url: URL?, contentMode: ContentMode = .fill) {
        self.url = url
        self.contentMode = contentMode
    }

    var body: some View {
        Group {
            if let image = loader.image {
                Image(uiImage: image)
                    .resizable()
                    .aspectRatio(contentMode: contentMode)
            } else if loader.isLoading {
                ProgressView()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                // Placeholder
                Rectangle()
                    .fill(Color.gray.opacity(0.2))
                    .overlay(
                        Image(systemName: "photo")
                            .foregroundColor(.gray)
                    )
            }
        }
        .onAppear {
            if let url = url {
                loader.load(url: url)
            }
        }
        .onDisappear {
            loader.cancel()
        }
        .onChange(of: url) { newURL in
            if let newURL = newURL {
                loader.load(url: newURL)
            }
        }
    }
}

/// Convenience view for loading recipe thumbnails by photo ID
struct RecipeThumbnail: View {
    let photoId: UUID?
    let size: CGFloat

    init(photoId: UUID?, size: CGFloat = 60) {
        self.photoId = photoId
        self.size = size
    }

    var body: some View {
        AuthenticatedImage(url: thumbnailURL)
            .frame(width: size, height: size)
            .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    private var thumbnailURL: URL? {
        guard let photoId = photoId,
              let baseURL = RamekinAPI.shared.serverURL else {
            return nil
        }
        return URL(string: "\(baseURL)/api/photos/\(photoId.uuidString)/thumbnail")
    }
}

#Preview {
    VStack {
        RecipeThumbnail(photoId: nil, size: 100)
        RecipeThumbnail(photoId: UUID(), size: 100)
    }
    .padding()
}
