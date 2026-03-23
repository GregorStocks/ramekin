import SwiftUI

struct RecipeListFilterBar: View {
    let availableTags: [TagItem]
    let selectedTags: Set<String>
    let photoFilter: PhotoFilter
    let advancedFilterLabel: String
    let hasAdvancedFilters: Bool
    let hasActiveFilters: Bool
    let onSelectPhotoFilter: (PhotoFilter) -> Void
    let onOpenAdvancedFilters: () -> Void
    let onToggleTag: (String) -> Void
    let onClearFilters: () -> Void

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                photoFilterMenu
                advancedFiltersButton

                ForEach(availableTags) { tag in
                    Button {
                        onToggleTag(tag.name)
                    } label: {
                        chipView(
                            text: tag.name,
                            isSelected: selectedTags.contains(tag.name)
                        )
                    }
                    .buttonStyle(.plain)
                }

                if hasActiveFilters {
                    Button {
                        onClearFilters()
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundColor(.secondary)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal)
            .padding(.vertical, 8)
        }
    }

    private var advancedFiltersButton: some View {
        Button {
            onOpenAdvancedFilters()
        } label: {
            chipView(
                text: advancedFilterLabel,
                icon: "line.3.horizontal.decrease.circle",
                isSelected: hasAdvancedFilters
            )
        }
        .buttonStyle(.plain)
    }

    private var photoFilterMenu: some View {
        Menu {
            ForEach(PhotoFilter.allCases, id: \.self) { filter in
                Button {
                    onSelectPhotoFilter(filter)
                } label: {
                    if photoFilter == filter {
                        Label(filter.label, systemImage: "checkmark")
                    } else {
                        Text(filter.label)
                    }
                }
            }
        } label: {
            chipView(
                text: photoFilter != .any ? photoFilter.label : nil,
                icon: "camera",
                isSelected: photoFilter != .any
            )
        }
    }

    private func chipView(text: String? = nil, icon: String? = nil, isSelected: Bool) -> some View {
        HStack(spacing: 4) {
            if let icon = icon {
                Image(systemName: icon)
                    .font(.caption)
            }
            if let text = text {
                Text(text)
                    .font(.caption)
                    .fontWeight(.medium)
                    .lineLimit(1)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(isSelected ? Color.orange : Color(.systemGray5))
        .foregroundColor(isSelected ? .white : .primary)
        .clipShape(Capsule())
    }
}
