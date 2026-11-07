import SwiftUI
import UIKit

/// SwiftUI sheet wrapper around UIActivityViewController. Use to share a
/// generated file (e.g. an export) once it's been written to a temp URL.
struct ShareSheet: UIViewControllerRepresentable {
    let activityItems: [Any]

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: activityItems, applicationActivities: nil)
    }

    func updateUIViewController(_ uiViewController: UIActivityViewController, context: Context) {}
}

/// Wraps a URL in an `Identifiable` so it can drive `.sheet(item:)`.
struct ShareItem: Identifiable {
    let id = UUID()
    let url: URL
}

/// How a server export ended: a temp file ready for the share sheet, or the
/// message for the "Export Failed" alert.
enum ExportOutcome {
    case ready(ShareItem)
    case failed(String)
    /// Cancelled, so there is nothing to show.
    case cancelled

    /// Downloads an export and writes it to a temp file. Shared by the
    /// single-recipe and all-recipes Paprika exports.
    static func download(_ fetch: () async throws -> RamekinAPI.ExportDownload) async -> ExportOutcome {
        do {
            let download = try await fetch()
            let url = try RecipeExportSupport.writeToTempFile(data: download.data, filename: download.filename)
            return .ready(ShareItem(url: url))
        } catch {
            if CancellationSupport.isCancellation(error) {
                return .cancelled
            }
            if let apiError = error as? RamekinAPI.APIError {
                return .failed(apiError.errorDescription ?? "Export failed")
            }
            return .failed(error.localizedDescription)
        }
    }
}

/// Adds a share-sheet presentation and an "Export Failed" alert to any view
/// driven by an export flow. Kept as a ViewModifier so the calling view's body
/// stays small enough for the Swift type-checker to handle.
struct ExportPresentationModifier: ViewModifier {
    @Binding var shareItem: ShareItem?
    @Binding var errorMessage: String?

    func body(content: Content) -> some View {
        content
            .sheet(item: $shareItem) { item in
                ShareSheet(activityItems: [item.url])
            }
            .alert("Export Failed", isPresented: Binding(
                get: { errorMessage != nil },
                set: { if !$0 { errorMessage = nil } }
            )) {
                Button("OK", role: .cancel) {}
            } message: {
                Text(errorMessage ?? "")
            }
    }
}
