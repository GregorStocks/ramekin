import SwiftUI

struct SettingsView: View {
    @EnvironmentObject var appState: AppState
    @StateObject private var model = SettingsViewModel()

    @State private var showingLogoutConfirmation = false
    @State private var showingClearLogsConfirmation = false
    @State private var showingDebugLogs = false
    @State private var debugLogs = ""

    var body: some View {
        Form {
            Section {
                VStack(spacing: 16) {
                    Image(systemName: "checkmark.circle.fill")
                        .scaledIconFont(size: 64)
                        .accessibilityHidden(true)
                        .foregroundColor(.green)

                    Text("You're all set!")
                        .font(.title2)
                        .fontWeight(.bold)

                    Text("Use the Share button in Safari to save recipes to Ramekin")
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                        .multilineTextAlignment(.center)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 24)
                .listRowBackground(Color.clear)
            }

            Section("How to Use") {
                HStack(spacing: 16) {
                    Image(systemName: "1.circle.fill")
                        .foregroundColor(.orange)
                        .font(.title2)
                    VStack(alignment: .leading) {
                        Text("Open a recipe in Safari")
                            .fontWeight(.medium)
                    }
                }
                .padding(.vertical, 4)

                HStack(spacing: 16) {
                    Image(systemName: "2.circle.fill")
                        .foregroundColor(.orange)
                        .font(.title2)
                    VStack(alignment: .leading) {
                        Text("Tap the Share button")
                            .fontWeight(.medium)
                        Text("The square with an arrow")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                }
                .padding(.vertical, 4)

                HStack(spacing: 16) {
                    Image(systemName: "3.circle.fill")
                        .foregroundColor(.orange)
                        .font(.title2)
                    VStack(alignment: .leading) {
                        Text("Choose \"Ramekin\"")
                            .fontWeight(.medium)
                        Text("You may need to scroll or tap \"More\"")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                }
                .padding(.vertical, 4)
            }

            Section("Library") {
                NavigationLink {
                    TagManagementView()
                } label: {
                    Label("Manage Tags", systemImage: "tag")
                }

                Button {
                    Task { await model.exportAllRecipes() }
                } label: {
                    HStack {
                        Label("Export All Recipes", systemImage: "square.and.arrow.up.on.square")
                        Spacer()
                        if model.isExportingAll {
                            ProgressView()
                        }
                    }
                }
                .disabled(model.isExportingAll)
            }

            Section("Account") {
                HStack {
                    Label("Username", systemImage: "person.fill")
                    Spacer()
                    Text(appState.username)
                        .foregroundColor(.secondary)
                }

                HStack {
                    Label("Server", systemImage: "server.rack")
                    Spacer()
                    Text(appState.serverURL)
                        .foregroundColor(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }

                HStack {
                    Label("Connection", systemImage: "wifi")
                    Spacer()
                    connectionStatusView
                }
            }

            Section {
                Button(role: .destructive) {
                    showingLogoutConfirmation = true
                } label: {
                    HStack {
                        Spacer()
                        Text("Sign Out")
                        Spacer()
                    }
                }
            }

            Section("Debug") {
                Button("View Debug Logs") {
                    debugLogs = DebugLogger.shared.readLogs()
                    showingDebugLogs = true
                }

                Button("Clear Logs", role: .destructive) {
                    showingClearLogsConfirmation = true
                }

                Button {
                    Task { await model.uploadLogs() }
                } label: {
                    HStack {
                        Label("Upload Logs to Server", systemImage: "icloud.and.arrow.up")
                        Spacer()
                        if model.logUpload == .uploading {
                            ProgressView()
                        }
                    }
                }
                .disabled(model.logUpload == .uploading)
            }
        }
        .sheet(isPresented: $showingDebugLogs) {
            NavigationStack {
                ScrollViewReader { proxy in
                    ScrollView {
                        Text(debugLogs.isEmpty ? "No logs yet." : debugLogs)
                            .font(.system(.caption, design: .monospaced))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding()
                        Color.clear.frame(height: 1).id("bottom")
                    }
                    .onAppear {
                        proxy.scrollTo("bottom")
                    }
                }
                .navigationTitle("Debug Logs")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .navigationBarLeading) {
                        if !debugLogs.isEmpty {
                            ShareLink(item: debugLogs) {
                                Image(systemName: "square.and.arrow.up")
                            }
                            .accessibilityLabel("Share Logs")
                        }
                    }
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Done") {
                            showingDebugLogs = false
                        }
                    }
                }
            }
        }
        .navigationTitle("Settings")
        .confirmationDialog(
            "Clear all debug logs?",
            isPresented: $showingClearLogsConfirmation,
            titleVisibility: .visible
        ) {
            Button("Clear Logs", role: .destructive) {
                DebugLogger.shared.clearLogs()
                debugLogs = ""
            }
        }
        .confirmationDialog(
            "Sign out of Ramekin?",
            isPresented: $showingLogoutConfirmation,
            titleVisibility: .visible
        ) {
            Button("Sign Out", role: .destructive) {
                appState.logout()
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("You'll need to sign in again to save recipes.")
        }
        // .task is cancelled when the screen goes away, unlike a Task
        // started from onAppear.
        .task {
            await model.checkConnection()
        }
        .refreshable {
            await model.checkConnection()
        }
        .modifier(ExportPresentationModifier(
            shareItem: $model.exportShareItem,
            errorMessage: $model.exportError
        ))
        .modifier(LogUploadPresentationModifier(
            state: model.logUpload,
            onDismiss: { model.dismissLogUploadResult() }
        ))
    }

}

// MARK: - Actions

extension SettingsView {
    @ViewBuilder
    private var connectionStatusView: some View {
        switch model.connectionStatus {
        case .unknown:
            Text("Unknown")
                .foregroundColor(.secondary)
        case .checking:
            ProgressView()
                .scaleEffect(0.8)
        case .connected:
            HStack(spacing: 4) {
                Image(systemName: "checkmark.circle.fill")
                    .foregroundColor(.green)
                Text("Connected")
                    .foregroundColor(.green)
            }
        case .failed(let error):
            HStack(spacing: 4) {
                Image(systemName: "xmark.circle.fill")
                    .foregroundColor(.red)
                Text(error)
                    .foregroundColor(.red)
                    .lineLimit(1)
            }
        }
    }
}

/// Adds the "Logs Uploaded" and "Upload Failed" alerts for the debug log
/// upload flow. Kept as a ViewModifier so the calling view's body stays small
/// enough for the Swift type-checker to handle.
struct LogUploadPresentationModifier: ViewModifier {
    let state: LogUploadState
    let onDismiss: @MainActor () -> Void

    private var errorMessage: String? {
        if case let .failed(message) = state { return message }
        return nil
    }

    func body(content: Content) -> some View {
        content
            .alert("Logs Uploaded", isPresented: Binding(
                get: { state == .succeeded },
                set: { if !$0 { onDismiss() } }
            )) {
                Button("OK", role: .cancel) {}
            }
            .alert("Upload Failed", isPresented: Binding(
                get: { errorMessage != nil },
                set: { if !$0 { onDismiss() } }
            )) {
                Button("OK", role: .cancel) {}
            } message: {
                Text(errorMessage ?? "")
            }
    }
}

#Preview {
    NavigationStack {
        SettingsView()
    }
    .environmentObject(AppState())
}
