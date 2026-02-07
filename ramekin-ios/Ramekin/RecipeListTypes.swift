import SwiftUI

// MARK: - Sort & Filter Types

enum RecipeSortOrder: String, CaseIterable {
    case newest
    case oldest
    case rating
    case title
    case created
    case random

    var sortBy: SortBy {
        switch self {
        case .newest, .oldest: return .updatedAt
        case .rating: return .rating
        case .title: return .title
        case .created: return .createdAt
        case .random: return .random
        }
    }

    var sortDir: Direction {
        switch self {
        case .newest, .rating, .created: return .desc
        case .oldest, .title: return .asc
        case .random: return .desc
        }
    }

    var label: String {
        switch self {
        case .newest: return "Newest first"
        case .oldest: return "Oldest first"
        case .rating: return "Highest rated"
        case .title: return "Title A–Z"
        case .created: return "Date added"
        case .random: return "Random"
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
                    Text(recipe.tags.joined(separator: ", "))
                        .font(.caption)
                        .foregroundColor(.orange)
                        .lineLimit(1)
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
}

#Preview {
    NavigationStack {
        RecipeListView()
    }
    .environmentObject(AppState())
}
