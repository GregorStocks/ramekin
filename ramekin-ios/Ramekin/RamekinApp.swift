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
    @Published var pendingRecipeId: UUID?

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

    func handleUniversalLink(_ url: URL) {
        guard let components = URLComponents(url: url, resolvingAgainstBaseURL: false) else {
            return
        }
        let parts = components.path.split(separator: "/", omittingEmptySubsequences: true)
        guard parts.count == 2, parts[0] == "recipes", let id = UUID(uuidString: String(parts[1])) else {
            return
        }
        pendingRecipeId = id
    }
}

enum Tab: Hashable {
    case recipes
    case mealPlan
    case shopping
}

struct ContentView: View {
    @EnvironmentObject var appState: AppState
    @State private var selectedTab: Tab = .recipes

    var body: some View {
        Group {
            if appState.isLoggedIn {
                TabView(selection: $selectedTab) {
                    RecipesTab()
                        .tabItem {
                            Label("Recipes", systemImage: "book")
                        }
                        .tag(Tab.recipes)

                    MealPlanView()
                        .tabItem {
                            Label("Meal Plan", systemImage: "calendar")
                        }
                        .tag(Tab.mealPlan)

                    ShoppingListView()
                        .tabItem {
                            Label("Shopping", systemImage: "cart")
                        }
                        .tag(Tab.shopping)
                }
                .onChange(of: appState.pendingRecipeId) { id in
                    if id != nil { selectedTab = .recipes }
                }
            } else {
                LoginView()
            }
        }
        .onContinueUserActivity(NSUserActivityTypeBrowsingWeb) { activity in
            guard let url = activity.webpageURL else { return }
            appState.handleUniversalLink(url)
        }
    }
}

struct RecipesTab: View {
    @EnvironmentObject var appState: AppState
    @State private var path = NavigationPath()

    var body: some View {
        NavigationStack(path: $path) {
            RecipeListView()
                .navigationDestination(for: NavigationDestination.self) { destination in
                    switch destination {
                    case .recipe(let id):
                        RecipeDetailView(recipeId: id)
                    case .settings:
                        SettingsView()
                    case .createRecipe:
                        RecipeFormView(mode: .create)
                    case .editRecipe(let id):
                        RecipeFormView(mode: .edit(recipeId: id))
                    }
                }
        }
        .onAppear { consumePendingRecipeId() }
        .onChange(of: appState.pendingRecipeId) { _ in consumePendingRecipeId() }
    }

    private func consumePendingRecipeId() {
        guard let id = appState.pendingRecipeId else { return }
        path.append(NavigationDestination.recipe(id))
        appState.pendingRecipeId = nil
    }
}

#Preview {
    ContentView()
        .environmentObject(AppState())
}
