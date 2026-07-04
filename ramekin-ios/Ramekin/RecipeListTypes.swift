import SwiftUI

// MARK: - Sort & Filter Types

enum RecipeSortOrder: String, CaseIterable {
    case newest
    case oldest
    case rating
    case title
    case created
    case random

    /// nil means "let the server pick": relevance when the query has text
    /// terms, newest-first otherwise.
    var sortBy: SortBy? {
        switch self {
        case .newest: return nil
        case .oldest: return .updatedAt
        case .rating: return .rating
        case .title: return .title
        case .created: return .createdAt
        case .random: return .random
        }
    }

    var sortDir: Direction? {
        switch self {
        case .newest: return nil
        case .rating, .created: return .desc
        case .oldest, .title: return .asc
        case .random: return .desc
        }
    }

    func label(searching: Bool) -> String {
        switch self {
        case .newest: return searching ? "Best match" : "Newest first"
        case .oldest: return "Oldest first"
        case .rating: return "Highest rated"
        case .title: return "Title A–Z"
        case .created: return "Date added"
        case .random: return "Random"
        }
    }
}

struct RecipeSortMenu: View {
    @Binding var sortOrder: RecipeSortOrder
    let searching: Bool
    let onChange: () -> Void

    var body: some View {
        Menu {
            ForEach(RecipeSortOrder.allCases, id: \.self) { order in
                Button {
                    sortOrder = order
                    onChange()
                } label: {
                    let label = order.label(searching: searching)
                    if sortOrder == order {
                        Label(label, systemImage: "checkmark")
                    } else {
                        Text(label)
                    }
                }
            }
        } label: {
            Image(systemName: "arrow.up.arrow.down")
        }
    }
}

enum PhotoFilter: String, CaseIterable {
    case any
    case hasPhotos
    case noPhotos

    var label: String {
        switch self {
        case .any: return "Any"
        case .hasPhotos: return "Has photos"
        case .noPhotos: return "No photos"
        }
    }
}

// MARK: - Recipe Row View

struct RecipeRowView: View {
    let recipe: RecipeSummary

    var body: some View {
        HStack(spacing: 12) {
            RecipeThumbnail(photoId: recipe.thumbnailPhotoId, size: 60)

            VStack(alignment: .leading, spacing: 4) {
                Text(recipe.title)
                    .font(.headline)
                    .lineLimit(2)

                if let description = recipe.description, !description.isEmpty {
                    Text(description)
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                        .lineLimit(2)
                }

                if !recipe.tags.isEmpty {
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 6) {
                            ForEach(Array(recipe.tags.prefix(3)), id: \.self) { tag in
                                HierarchicalTagChip(name: tag, valueFont: .caption2, namespaceFont: .caption2)
                            }
                        }
                    }
                }
            }
        }
        .padding(.vertical, 4)
    }
}

// MARK: - Navigation Destinations

enum NavigationDestination: Hashable {
    case recipe(UUID)
    case settings
    case createRecipe
    case editRecipe(UUID)
}

#Preview {
    NavigationStack {
        RecipeListView()
    }
    .environmentObject(AppState())
}
