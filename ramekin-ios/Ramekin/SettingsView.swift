import SwiftUI

struct SettingsView: View {
    @EnvironmentObject var appState: AppState

    @State private var showingLogoutConfirmation = false
    @State private var connectionStatus: ConnectionStatus = .unknown
    @State private var showingDebugLogs = false
    @State private var debugLogs = ""

    enum ConnectionStatus {
        case unknown
        case checking
        case connected
        case failed(String)
    }

    var body: some View {
        Form {
            Section {
                VStack(spacing: 16) {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.system(size: 64))
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
                Button("View Share Extension Logs") {
                    debugLogs = DebugLogger.shared.readLogs()
                    showingDebugLogs = true
                }

                Button("Clear Logs") {
                    DebugLogger.shared.clearLogs()
                    debugLogs = ""
                }
            }
        }
        .sheet(isPresented: $showingDebugLogs) {
            NavigationStack {
                ScrollView {
                    Text(debugLogs.isEmpty ? "No logs yet. Try using the share extension." : debugLogs)
                        .font(.system(.caption, design: .monospaced))
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding()
                }
                .navigationTitle("Extension Logs")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Done") {
                            showingDebugLogs = false
                        }
                    }
                }
            }
        }
        .navigationTitle("Ramekin")
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
        .onAppear {
            checkConnection()
        }
        .refreshable {
            await checkConnectionAsync()
        }
    }

    @ViewBuilder
    private var connectionStatusView: some View {
        switch connectionStatus {
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

    private func checkConnection() {
        Task {
            await checkConnectionAsync()
        }
    }

    private func checkConnectionAsync() async {
        await MainActor.run {
            connectionStatus = .checking
        }

        do {
            let connected = try await RamekinAPI.shared.testConnection()
            await MainActor.run {
                connectionStatus = connected ? .connected : .failed("Not reachable")
            }
        } catch {
            await MainActor.run {
                connectionStatus = .failed("Offline")
            }
        }
    }
}

#Preview {
    NavigationStack {
        SettingsView()
    }
    .environmentObject(AppState())
}
