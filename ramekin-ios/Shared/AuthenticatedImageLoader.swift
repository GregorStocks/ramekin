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

/// The server answered with bytes that are not an image.
struct UndecodableImageError: Error, LocalizedError {
    var errorDescription: String? { "The image could not be decoded" }
}

/// Loads images from authenticated endpoints with Bearer token
@MainActor
class AuthenticatedImageLoader: ObservableObject {
    typealias TokenProvider = @MainActor () -> String?
    typealias ImageFetcher = @Sendable (URLRequest) async throws -> (Data, URLResponse)

    /// Exactly one of: nothing shown, loading, an image, or a failure.
    enum Phase {
        case idle
        case loading
        case loaded(UIImage)
        case failed(Error)
    }

    @Published private(set) var phase: Phase = .idle

    var image: UIImage? {
        if case let .loaded(image) = phase { return image }
        return nil
    }

    var isLoading: Bool {
        if case .loading = phase { return true }
        return false
    }

    var error: Error? {
        if case let .failed(error) = phase { return error }
        return nil
    }

    private let imageCache: AuthenticatedImageCache
    private let tokenProvider: TokenProvider
    private let imageFetcher: ImageFetcher
    /// The in-flight load. Readable so tests can await a load instead of polling for its result.
    private(set) var currentTask: Task<Void, Never>?
    /// Identifies the latest load (or cancel): only it may change `phase`.
    private var generation = UUID()
    /// The URL `phase` describes, so reloading the same URL (the view
    /// reappearing) keeps showing its image instead of blanking it.
    private var phaseURL: URL?

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
        currentTask?.cancel()
        let generation = UUID()
        self.generation = generation

        // A different picture must not show the old one while it loads.
        if phaseURL != url {
            phase = .idle
        }
        phaseURL = url

        guard let token = tokenProvider() else {
            currentTask = nil
            phase = .idle
            return
        }

        let cacheKey = Self.cacheKey(for: url, token: token)

        currentTask = Task {
            await self.run(url: url, token: token, cacheKey: cacheKey, generation: generation)
        }
    }

    /// One load. Every step after an await first checks it is still the
    /// latest load, so a superseded or cancelled one never touches `phase`.
    private func run(url: URL, token: String, cacheKey: String, generation: UUID) async {
        if let cachedImage = await imageCache.image(forKey: cacheKey) {
            guard self.generation == generation else { return }
            phase = .loaded(cachedImage)
            return
        }

        guard self.generation == generation else { return }
        if image == nil {
            phase = .loading
        }

        let result: Phase
        do {
            result = .loaded(try await fetchImage(url: url, token: token, cacheKey: cacheKey))
        } catch {
            guard self.generation == generation else { return }
            if CancellationSupport.isCancellation(error) || Task.isCancelled {
                settleCancelled()
                return
            }
            // Refetching the picture already on screen: keep showing it.
            if image != nil { return }
            result = .failed(error)
        }

        guard self.generation == generation else { return }
        phase = result
    }

    private func fetchImage(url: URL, token: String, cacheKey: String) async throws -> UIImage {
        var request = URLRequest(url: url)
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        RamekinAPI.shared.applyAccessHeaders(to: &request)

        let (data, response) = try await imageFetcher(request)
        if let httpResponse = response as? HTTPURLResponse {
            AuthExpiry.notifyIfRejected(statusCode: httpResponse.statusCode, sentToken: token)
        }
        guard let httpResponse = response as? HTTPURLResponse,
              httpResponse.statusCode == 200 else {
            throw URLError(.badServerResponse)
        }
        guard let loadedImage = UIImage(data: data) else {
            throw UndecodableImageError()
        }
        await imageCache.insert(loadedImage, forKey: cacheKey)
        return loadedImage
    }

    func cancel() {
        currentTask?.cancel()
        currentTask = nil
        generation = UUID()
        settleCancelled()
    }

    /// A cancelled load leaves an image it already had, and nothing else.
    private func settleCancelled() {
        if case .loading = phase {
            phase = .idle
        }
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
