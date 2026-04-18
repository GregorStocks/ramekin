import Foundation
import os.log
import UniformTypeIdentifiers

private let sharedPagePayloadLogger = Logger(
    subsystem: "com.ramekin.app.share",
    category: "SharedPagePayloadExtractor"
)

/// Payload extracted from a Safari share extension's JavaScript preprocessing
/// file. Mirrors what `SharePreprocessor.js` puts in `completionFunction`.
struct SharedPagePayload: Equatable {
    let html: String
    let url: URL
    let title: String?
}

protocol SharedPagePayloadItemProvider {
    var registeredTypeIdentifiers: [String] { get }
    func hasItemConformingToTypeIdentifier(_ typeIdentifier: String) -> Bool
    func loadItem(
        forTypeIdentifier typeIdentifier: String,
        completionHandler: @escaping @Sendable (NSSecureCoding?, Error?) -> Void
    )
}

// NSItemProvider already gets loadItem(forTypeIdentifier:completionHandler:) from
// SharedURLExtractor.swift's conformance extension. We just declare it conforms here.
extension NSItemProvider: SharedPagePayloadItemProvider {}

enum SharedPagePayloadExtractor {
    static func extractPayload(
        from extensionItems: [NSExtensionItem]
    ) async -> SharedPagePayload? {
        let providers = extensionItems.flatMap { item in
            item.attachments?.map { $0 as any SharedPagePayloadItemProvider } ?? []
        }
        return await extractPayload(from: providers)
    }

    static func extractPayload(
        from providers: [any SharedPagePayloadItemProvider]
    ) async -> SharedPagePayload? {
        for provider in providers {
            sharedPagePayloadLogger.debug(
                "Provider registered types: \(provider.registeredTypeIdentifiers)"
            )
            if let payload = await extractPayload(from: provider) {
                return payload
            }
        }
        sharedPagePayloadLogger.error("No provider yielded a SharedPagePayload")
        return nil
    }

    private static func extractPayload(
        from provider: any SharedPagePayloadItemProvider
    ) async -> SharedPagePayload? {
        guard provider.hasItemConformingToTypeIdentifier(UTType.propertyList.identifier) else {
            return nil
        }

        let (item, error) = await loadItem(
            from: provider,
            typeIdentifier: UTType.propertyList.identifier
        )
        if let error {
            sharedPagePayloadLogger.error(
                "Error loading property list: \(error.localizedDescription)"
            )
            return nil
        }

        guard let dict = item as? NSDictionary,
              let results = dict[NSExtensionJavaScriptPreprocessingResultsKey] as? [String: Any]
        else {
            sharedPagePayloadLogger.error("No preprocessing results key in payload")
            return nil
        }

        guard let html = results["html"] as? String, !html.isEmpty,
              let urlString = results["url"] as? String,
              let url = URL(string: urlString)
        else {
            sharedPagePayloadLogger.error("Preprocessing results missing html or url")
            return nil
        }

        let title = results["title"] as? String
        return SharedPagePayload(html: html, url: url, title: title)
    }

    private static func loadItem(
        from provider: any SharedPagePayloadItemProvider,
        typeIdentifier: String
    ) async -> (NSSecureCoding?, Error?) {
        await withCheckedContinuation { continuation in
            provider.loadItem(forTypeIdentifier: typeIdentifier) { item, error in
                continuation.resume(returning: (item, error))
            }
        }
    }
}
