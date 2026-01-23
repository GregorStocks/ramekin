import UIKit
import SwiftUI
import UniformTypeIdentifiers

/// Share Extension entry point
/// Handles receiving URLs from Safari and other apps
class ShareViewController: UIViewController {

    override func viewDidLoad() {
        super.viewDidLoad()

        // Extract the shared URL
        extractURL { [weak self] url in
            DispatchQueue.main.async {
                self?.presentShareView(with: url)
            }
        }
    }

    private func presentShareView(with url: URL?) {
        let shareView = ShareExtensionView(
            sharedURL: url,
            onComplete: { [weak self] in
                self?.extensionContext?.completeRequest(returningItems: nil)
            },
            onCancel: { [weak self] in
                self?.extensionContext?.cancelRequest(withError: NSError(
                    domain: "com.ramekin.share",
                    code: 0,
                    userInfo: [NSLocalizedDescriptionKey: "User cancelled"]
                ))
            }
        )

        let hostingController = UIHostingController(rootView: shareView)
        hostingController.view.backgroundColor = .systemBackground

        addChild(hostingController)
        view.addSubview(hostingController.view)

        hostingController.view.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            hostingController.view.topAnchor.constraint(equalTo: view.topAnchor),
            hostingController.view.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            hostingController.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            hostingController.view.trailingAnchor.constraint(equalTo: view.trailingAnchor)
        ])

        hostingController.didMove(toParent: self)
    }

    /// Extract URL from the share extension context
    private func extractURL(completion: @escaping (URL?) -> Void) {
        guard let extensionItems = extensionContext?.inputItems as? [NSExtensionItem] else {
            completion(nil)
            return
        }

        for item in extensionItems {
            guard let attachments = item.attachments else { continue }

            for provider in attachments {
                // Try to get URL directly
                if provider.hasItemConformingToTypeIdentifier(UTType.url.identifier) {
                    provider.loadItem(forTypeIdentifier: UTType.url.identifier) { item, _ in
                        if let url = item as? URL {
                            completion(url)
                            return
                        }
                    }
                    return
                }

                // Try to get plain text (might be a URL string)
                if provider.hasItemConformingToTypeIdentifier(UTType.plainText.identifier) {
                    provider.loadItem(forTypeIdentifier: UTType.plainText.identifier) { item, _ in
                        if let text = item as? String, let url = URL(string: text) {
                            completion(url)
                            return
                        }
                    }
                    return
                }
            }
        }

        completion(nil)
    }
}

/// SwiftUI View for the Share Extension
struct ShareExtensionView: View {
    let sharedURL: URL?
    let onComplete: () -> Void
    let onCancel: () -> Void

    @State private var status: ShareStatus = .ready
    @State private var errorMessage: String?

    enum ShareStatus {
        case ready
        case sending
        case success
        case error
        case notLoggedIn
    }

    var body: some View {
        NavigationStack {
            VStack(spacing: 24) {
                statusIcon
                    .font(.system(size: 64))
                    .padding(.top, 32)

                statusText

                if let url = sharedURL {
                    Text(url.absoluteString)
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .lineLimit(2)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal)
                }

                Spacer()

                actionButton
                    .padding(.horizontal)
                    .padding(.bottom, 32)
            }
            .navigationTitle("Save to Ramekin")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        onCancel()
                    }
                }
            }
            .onAppear {
                checkLoginAndSend()
            }
        }
    }

    @ViewBuilder
    private var statusIcon: some View {
        switch status {
        case .ready, .sending:
            ProgressView()
                .scaleEffect(1.5)
        case .success:
            Image(systemName: "checkmark.circle.fill")
                .foregroundColor(.green)
        case .error:
            Image(systemName: "xmark.circle.fill")
                .foregroundColor(.red)
        case .notLoggedIn:
            Image(systemName: "person.crop.circle.badge.exclamationmark.fill")
                .foregroundColor(.orange)
        }
    }

    @ViewBuilder
    private var statusText: some View {
        switch status {
        case .ready:
            Text("Preparing...")
                .font(.title2)
        case .sending:
            VStack(spacing: 8) {
                Text("Saving Recipe...")
                    .font(.title2)
                Text("The recipe will be processed in the background")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
            }
        case .success:
            VStack(spacing: 8) {
                Text("Saved!")
                    .font(.title2)
                    .fontWeight(.bold)
                Text("The recipe is being imported")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
            }
        case .error:
            VStack(spacing: 8) {
                Text("Failed to Save")
                    .font(.title2)
                    .fontWeight(.bold)
                if let error = errorMessage {
                    Text(error)
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                        .multilineTextAlignment(.center)
                }
            }
        case .notLoggedIn:
            VStack(spacing: 8) {
                Text("Not Signed In")
                    .font(.title2)
                    .fontWeight(.bold)
                Text("Open the Ramekin app to sign in first")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
            }
        }
    }

    @ViewBuilder
    private var actionButton: some View {
        switch status {
        case .ready, .sending:
            EmptyView()
        case .success:
            Button {
                onComplete()
            } label: {
                Text("Done")
                    .fontWeight(.semibold)
                    .frame(maxWidth: .infinity)
                    .padding()
                    .background(Color.green)
                    .foregroundColor(.white)
                    .cornerRadius(12)
            }
        case .error:
            VStack(spacing: 12) {
                Button {
                    checkLoginAndSend()
                } label: {
                    Text("Try Again")
                        .fontWeight(.semibold)
                        .frame(maxWidth: .infinity)
                        .padding()
                        .background(Color.orange)
                        .foregroundColor(.white)
                        .cornerRadius(12)
                }

                Button {
                    onCancel()
                } label: {
                    Text("Cancel")
                        .foregroundColor(.secondary)
                }
            }
        case .notLoggedIn:
            Button {
                onCancel()
            } label: {
                Text("OK")
                    .fontWeight(.semibold)
                    .frame(maxWidth: .infinity)
                    .padding()
                    .background(Color.orange)
                    .foregroundColor(.white)
                    .cornerRadius(12)
            }
        }
    }

    private func checkLoginAndSend() {
        // Check if logged in
        guard RamekinAPI.shared.isLoggedIn else {
            status = .notLoggedIn
            return
        }

        // Check if we have a URL
        guard let url = sharedURL else {
            status = .error
            errorMessage = "No URL to save"
            return
        }

        // Send the URL
        sendURL(url)
    }

    private func sendURL(_ url: URL) {
        status = .sending

        Task {
            do {
                _ = try await RamekinAPI.shared.scrapeURL(url.absoluteString)

                await MainActor.run {
                    status = .success

                    // Auto-dismiss after a short delay
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
                        onComplete()
                    }
                }
            } catch {
                await MainActor.run {
                    status = .error
                    errorMessage = error.localizedDescription
                }
            }
        }
    }
}

#Preview {
    ShareExtensionView(
        sharedURL: URL(string: "https://example.com/recipe"),
        onComplete: {},
        onCancel: {}
    )
}
