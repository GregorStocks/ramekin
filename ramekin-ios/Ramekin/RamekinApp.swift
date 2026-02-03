import SwiftUI

@main
struct RamekinApp: App {
    @StateObject private var appState = AppState()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(appState)
        }
    }
}

/// Global app state shared across views
class AppState: ObservableObject {
    @Published var isLoggedIn: Bool = false
    @Published var username: String = ""
    @Published var serverURL: String = ""

    init() {
        refreshState()
    }

    func refreshState() {
        isLoggedIn = RamekinAPI.shared.isLoggedIn
        username = KeychainHelper.shared.getUsername() ?? ""
        serverURL = KeychainHelper.shared.getServerURL() ?? ""
    }

    func logout() {
        RamekinAPI.shared.logout()
        refreshState()
    }
}

struct ContentView: View {
    @EnvironmentObject var appState: AppState

    var body: some View {
        if appState.isLoggedIn {
            TabView {
                RecipesTab()
                    .tabItem {
                        Label("Recipes", systemImage: "book")
                    }

                ShoppingListView()
                    .tabItem {
                        Label("Shopping", systemImage: "cart")
                    }
            }
        } else {
            LoginView()
        }
    }
}

struct RecipesTab: View {
    var body: some View {
        NavigationStack {
            RecipeListView()
                .navigationDestination(for: NavigationDestination.self) { destination in
                    switch destination {
                    case .recipe(let id):
                        RecipeDetailView(recipeId: id)
                    case .settings:
                        SettingsView()
                    }
                }
        }
    }
}

#Preview {
    ContentView()
        .environmentObject(AppState())
}
