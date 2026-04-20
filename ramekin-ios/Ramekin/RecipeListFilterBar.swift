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
    @State private var showingTagFilters = false

    private var selectedTagNames: [String] {
        selectedTags.sorted()
    }

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                photoFilterMenu
                advancedFiltersButton
                tagFiltersButton

                ForEach(selectedTagNames, id: \.self) { tagName in
                    Button {
                        onToggleTag(tagName)
                    } label: {
                        HierarchicalTagChip(name: tagName, isSelected: true)
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
        .sheet(isPresented: $showingTagFilters) {
            RecipeTagFiltersSheet(
                availableTags: availableTags,
                selectedTags: selectedTags,
                onToggleTag: onToggleTag
            )
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

    private var tagFiltersButton: some View {
        Button {
            showingTagFilters = true
        } label: {
            chipView(
                text: selectedTags.isEmpty ? "Tags" : "\(selectedTags.count) tag\(selectedTags.count == 1 ? "" : "s")",
                icon: "tag",
                isSelected: !selectedTags.isEmpty
            )
        }
        .buttonStyle(.plain)
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

private struct RecipeTagFiltersSheet: View {
    let availableTags: [TagItem]
    let selectedTags: Set<String>
    let onToggleTag: (String) -> Void

    @Environment(\.dismiss) private var dismiss

    private var groupedTags: [TagHierarchySupport.TagGroup<TagItem>] {
        TagHierarchySupport.groups(for: availableTags)
    }

    var body: some View {
        NavigationStack {
            List {
                if groupedTags.isEmpty {
                    Section {
                        Text("No tags available yet.")
                            .foregroundColor(.secondary)
                    }
                } else {
                    ForEach(groupedTags) { group in
                        Section(group.title) {
                            tagGrid(group.items)
                        }
                    }
                }
            }
            .listStyle(.insetGrouped)
            .navigationTitle("Tag Filters")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Done") {
                        dismiss()
                    }
                }
            }
        }
    }

    private func tagGrid(_ tags: [TagItem]) -> some View {
        FlexibleTagGrid(items: tags, id: \.id) { tag in
            Button {
                onToggleTag(tag.name)
            } label: {
                HierarchicalTagChip(
                    name: tag.name,
                    isSelected: selectedTags.contains(tag.name)
                )
            }
            .buttonStyle(.plain)
        }
    }
}

struct FlexibleTagGrid<Item, ID: Hashable, Content: View>: View {
    let items: [Item]
    let id: KeyPath<Item, ID>
    let content: (Item) -> Content

    private let columns = [
        GridItem(.adaptive(minimum: 110), alignment: .leading)
    ]

    var body: some View {
        LazyVGrid(columns: columns, alignment: .leading, spacing: 10) {
            ForEach(items, id: id) { item in
                content(item)
            }
        }
        .padding(.vertical, 4)
    }
}
