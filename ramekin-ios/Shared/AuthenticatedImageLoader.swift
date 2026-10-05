import SwiftUI

actor AuthenticatedImageCache {
    static let shared = AuthenticatedImageCache()

    private let storage = NSCache<NSString, UIImage>()

    func image(forKey key: String) -> UIImage? {
        storage.object(forKey: key as NSString)
    }

    func insert(_ image: UIImage, forKey key: String) {
        storage.setObject(image, forKey: key as NSString)
    }

    func removeAll() {
        storage.removeAllObjects()
    }
}

/// Loads images from authenticated endpoints with Bearer token
@MainActor
class AuthenticatedImageLoader: ObservableObject {
    typealias TokenProvider = @MainActor () -> String?
    typealias ImageFetcher = @Sendable (URLRequest) async throws -> (Data, URLResponse)

    @Published var image: UIImage?
    @Published var isLoading = false
    @Published var error: Error?

    private let imageCache: AuthenticatedImageCache
    private let tokenProvider: TokenProvider
    private let imageFetcher: ImageFetcher
    /// The in-flight load. Readable so tests can await a load instead of polling for its result.
    private(set) var currentTask: Task<Void, Never>?

    init(
        imageCache: AuthenticatedImageCache = .shared,
        tokenProvider: @escaping TokenProvider = { RamekinAPI.shared.authToken },
        imageFetcher: @escaping ImageFetcher = { request in
            try await insecureSession.data(for: request)
        }
    ) {
        self.imageCache = imageCache
        self.tokenProvider = tokenProvider
        self.imageFetcher = imageFetcher
    }

    func load(url: URL) {
        // Cancel any existing load
        currentTask?.cancel()

        guard let token = tokenProvider() else {
            return
        }

        error = nil
        let cacheKey = Self.cacheKey(for: url, token: token)

        currentTask = Task {
            if let cachedImage = await imageCache.image(forKey: cacheKey) {
                guard !Task.isCancelled else { return }

                self.image = cachedImage
                self.isLoading = false
                return
            }

            guard !Task.isCancelled else {
                self.isLoading = false
                return
            }
            self.isLoading = true

            var request = URLRequest(url: url)
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
            RamekinAPI.shared.applyAccessHeaders(to: &request)

            do {
                let (data, response) = try await imageFetcher(request)

                guard !Task.isCancelled else { return }

                guard let httpResponse = response as? HTTPURLResponse,
                      httpResponse.statusCode == 200 else {
                    throw URLError(.badServerResponse)
                }

                if let loadedImage = UIImage(data: data) {
                    await imageCache.insert(loadedImage, forKey: cacheKey)
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
        isLoading = false
    }

    static func cacheKey(for url: URL, token: String) -> String {
        "\(token)|\(url.absoluteString)"
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
