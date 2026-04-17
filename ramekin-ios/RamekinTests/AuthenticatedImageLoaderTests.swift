import UIKit
import XCTest
@testable import Ramekin

final class AuthenticatedImageLoaderTests: XCTestCase {
    @MainActor
    func testLoadUsesCachedImageWithoutFetching() async throws {
        let url = try XCTUnwrap(URL(string: "https://example.com/image.png"))
        let token = "token-a"
        let cachedImage = makeImage(color: .red)
        let cache = AuthenticatedImageCache()
        let fetchSpy = FetchSpy()

        await cache.insert(
            cachedImage,
            forKey: AuthenticatedImageLoader.cacheKey(for: url, token: token)
        )

        let loader = AuthenticatedImageLoader(
            imageCache: cache,
            tokenProvider: { token },
            imageFetcher: { request in
                await fetchSpy.record(request: request)
                return (Data(), URLResponse())
            }
        )

        loader.load(url: url)
        try await waitForImage(on: loader)

        XCTAssertTrue(loader.image === cachedImage)
        XCTAssertFalse(loader.isLoading)
        XCTAssertNil(loader.error)
        XCTAssertEqual(await fetchSpy.count, 0)
    }

    @MainActor
    func testLoadScopesCacheEntriesByAuthToken() async throws {
        let url = try XCTUnwrap(URL(string: "https://example.com/image.png"))
        let cache = AuthenticatedImageCache()
        let fetcher = TokenAwareFetcher(
            responses: [
                "token-a": makeImageData(color: .red),
                "token-b": makeImageData(color: .blue)
            ]
        )

        let loaderA = AuthenticatedImageLoader(
            imageCache: cache,
            tokenProvider: { "token-a" },
            imageFetcher: fetcher.fetch
        )
        loaderA.load(url: url)
        try await waitForImage(on: loaderA)
        let tokenAFirstImageData = try XCTUnwrap(loaderA.image?.pngData())

        let loaderB = AuthenticatedImageLoader(
            imageCache: cache,
            tokenProvider: { "token-b" },
            imageFetcher: fetcher.fetch
        )
        loaderB.load(url: url)
        try await waitForImage(on: loaderB)
        let tokenBImageData = try XCTUnwrap(loaderB.image?.pngData())

        let loaderASecond = AuthenticatedImageLoader(
            imageCache: cache,
            tokenProvider: { "token-a" },
            imageFetcher: fetcher.fetch
        )
        loaderASecond.load(url: url)
        try await waitForImage(on: loaderASecond)
        let tokenASecondImageData = try XCTUnwrap(loaderASecond.image?.pngData())

        XCTAssertNotEqual(tokenAFirstImageData, tokenBImageData)
        XCTAssertEqual(tokenAFirstImageData, tokenASecondImageData)
        XCTAssertEqual(await fetcher.requestCount, 2)
    }

    @MainActor
    private func waitForImage(
        on loader: AuthenticatedImageLoader,
        timeoutNanoseconds: UInt64 = 1_000_000_000
    ) async throws {
        let deadline = ContinuousClock.now + .nanoseconds(Int64(timeoutNanoseconds))
        while loader.image == nil && loader.error == nil && ContinuousClock.now < deadline {
            try await Task.sleep(nanoseconds: 10_000_000)
        }

        XCTAssertNotNil(loader.image)
    }

    private func makeImage(color: UIColor) -> UIImage {
        let renderer = UIGraphicsImageRenderer(size: CGSize(width: 2, height: 2))
        return renderer.image { context in
            color.setFill()
            context.fill(CGRect(x: 0, y: 0, width: 2, height: 2))
        }
    }

    private func makeImageData(color: UIColor) -> Data {
        makeImage(color: color).pngData() ?? Data()
    }
}

private actor FetchSpy {
    private(set) var count = 0

    func record(request: URLRequest) {
        _ = request
        count += 1
    }
}

private actor TokenAwareFetcher {
    private let responses: [String: Data]
    private(set) var requestCount = 0

    init(responses: [String: Data]) {
        self.responses = responses
    }

    func fetch(request: URLRequest) throws -> (Data, URLResponse) {
        requestCount += 1

        let token = request.value(forHTTPHeaderField: "Authorization")?
            .replacingOccurrences(of: "Bearer ", with: "")
        guard let token, let data = responses[token] else {
            throw URLError(.userAuthenticationRequired)
        }

        let response = HTTPURLResponse(
            url: request.url ?? URL(fileURLWithPath: "/"),
            statusCode: 200,
            httpVersion: nil,
            headerFields: nil
        )!
        return (data, response)
    }
}
