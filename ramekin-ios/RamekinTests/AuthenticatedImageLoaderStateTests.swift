import UIKit
import XCTest
@testable import Ramekin

/// The loader shows exactly one of nothing, a spinner, an image, or a failure.
@MainActor
final class AuthenticatedImageLoaderStateTests: XCTestCase {
    private let firstURL = URL(string: "https://example.com/first.png")!
    private let secondURL = URL(string: "https://example.com/second.png")!

    func testNewURLDoesNotShowPreviousImageWhileLoading() async {
        let secondStarted = RequestGate()
        let releaseSecond = RequestGate()
        let imageData = Self.imageData()
        let firstURL = firstURL
        let loader = makeLoader { request in
            if request.url != firstURL {
                await secondStarted.open()
                await releaseSecond.wait()
            }
            return (imageData, Self.okResponse(for: request))
        }

        loader.load(url: firstURL)
        await loader.currentTask?.value
        XCTAssertNotNil(loader.image)

        loader.load(url: secondURL)
        let secondLoad = loader.currentTask
        await secondStarted.wait()
        XCTAssertNil(loader.image, "The first picture must not stand in for the second")
        XCTAssertTrue(loader.isLoading)

        releaseSecond.open()
        await secondLoad?.value
        XCTAssertNotNil(loader.image)
        XCTAssertFalse(loader.isLoading)
        XCTAssertNil(loader.error)
    }

    func testUndecodableDataIsAFailure() async {
        let loader = makeLoader { request in
            (Data("not an image".utf8), Self.okResponse(for: request))
        }

        loader.load(url: firstURL)
        await loader.currentTask?.value

        XCTAssertNil(loader.image)
        XCTAssertNotNil(loader.error)
        XCTAssertFalse(loader.isLoading)
    }

    func testFailureReplacesNothingButTheSpinner() async {
        let loader = makeLoader { _ in throw URLError(.timedOut) }

        loader.load(url: firstURL)
        await loader.currentTask?.value

        XCTAssertNil(loader.image)
        XCTAssertNotNil(loader.error)
        XCTAssertFalse(loader.isLoading)
    }

    func testCancelWhileFetchingClearsSpinner() async {
        let fetchStarted = RequestGate()
        let releaseFetch = RequestGate()
        let imageData = Self.imageData()
        let loader = makeLoader { request in
            await fetchStarted.open()
            await releaseFetch.wait()
            return (imageData, Self.okResponse(for: request))
        }

        loader.load(url: firstURL)
        let load = loader.currentTask
        await fetchStarted.wait()
        XCTAssertTrue(loader.isLoading)

        loader.cancel()
        releaseFetch.open()
        await load?.value

        XCTAssertFalse(loader.isLoading)
        XCTAssertNil(loader.image, "A cancelled load must not publish late")
        XCTAssertNil(loader.error)
    }

    func testSupersededLoadCannotPublish() async {
        let firstStarted = RequestGate()
        let releaseFirst = RequestGate()
        let firstURL = firstURL
        let imageData = Self.imageData()
        let loader = makeLoader { request in
            if request.url == firstURL {
                await firstStarted.open()
                await releaseFirst.wait()
                throw URLError(.timedOut)
            }
            return (imageData, Self.okResponse(for: request))
        }

        loader.load(url: firstURL)
        let firstLoad = loader.currentTask
        await firstStarted.wait()
        loader.load(url: secondURL)
        await loader.currentTask?.value
        releaseFirst.open()
        await firstLoad?.value

        XCTAssertNotNil(loader.image)
        XCTAssertNil(loader.error)
    }

    private func makeLoader(
        fetch: @escaping AuthenticatedImageLoader.ImageFetcher
    ) -> AuthenticatedImageLoader {
        AuthenticatedImageLoader(
            imageCache: AuthenticatedImageCache(),
            tokenProvider: { "token" },
            imageFetcher: fetch
        )
    }

    private nonisolated static func okResponse(for request: URLRequest) -> URLResponse {
        HTTPURLResponse(
            url: request.url ?? URL(fileURLWithPath: "/"),
            statusCode: 200,
            httpVersion: nil,
            headerFields: nil
        )!
    }

    private nonisolated static func imageData() -> Data {
        let renderer = UIGraphicsImageRenderer(size: CGSize(width: 2, height: 2))
        return renderer.image { context in
            UIColor.red.setFill()
            context.fill(CGRect(x: 0, y: 0, width: 2, height: 2))
        }.pngData() ?? Data()
    }
}
