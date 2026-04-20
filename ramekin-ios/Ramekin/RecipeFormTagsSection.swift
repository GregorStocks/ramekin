import SwiftUI

struct RecipeFormTagsSection: View {
    @Binding var tags: [String]
    @Binding var availableTags: [TagItem]
    @Binding var selectedTagNamespace: String?
    @Binding var newTagValue: String
    @Binding var newNamespace: String
    let onAddTag: () -> Void

    private var namespaceOptions: [String] {
        TagHierarchySupport.availableNamespaces(
            from: availableTags.map(\.name) + tags
        )
    }

    private var groupedAvailableTags: [TagHierarchySupport.TagGroup<TagItem>] {
        TagHierarchySupport.groups(for: availableTags)
    }

    private var groupedSelectedTags: [TagHierarchySupport.TagGroup<TagHierarchySupport.ParsedTag>] {
        TagHierarchySupport.groups(for: tags)
    }

    private var canAddTag: Bool {
        if selectedTagNamespace == nil {
            return TagHierarchySupport.normalizedValue(from: newTagValue) != nil
        }

        if selectedTagNamespace?.isEmpty == true {
            return TagHierarchySupport.normalizedNamespace(from: newNamespace) != nil
                && TagHierarchySupport.normalizedValue(from: newTagValue) != nil
        }

        return TagHierarchySupport.normalizedValue(from: newTagValue) != nil
    }

    var body: some View {
        Section("Tags") {
            if !groupedSelectedTags.isEmpty {
                ForEach(groupedSelectedTags) { group in
                    VStack(alignment: .leading, spacing: 8) {
                        Text(group.title)
                            .font(.caption)
                            .foregroundColor(.secondary)
                        selectedTagChips(group.items)
                    }
                }
            }

            VStack(alignment: .leading, spacing: 12) {
                namespacePicker

                if selectedTagNamespace?.isEmpty == true {
                    TextField("New namespace", text: $newNamespace)
                        .textInputAutocapitalization(.never)
                        .disableAutocorrection(true)
                }

                HStack {
                    TextField("Tag value", text: $newTagValue)
                        .autocapitalization(.none)
                        .autocorrectionDisabled()
                        .onSubmit {
                            if canAddTag {
                                onAddTag()
                            }
                        }
                    Button("Add") { onAddTag() }
                        .disabled(!canAddTag)
                }
            }

            if !groupedAvailableTags.isEmpty {
                ForEach(groupedAvailableTags) { group in
                    VStack(alignment: .leading, spacing: 8) {
                        Text(group.title)
                            .font(.caption)
                            .foregroundColor(.secondary)
                        availableTagChips(group.items)
                    }
                }
            }
        }
    }

    private var namespacePicker: some View {
        Menu {
            Button("No namespace") {
                selectedTagNamespace = nil
                newNamespace = ""
            }

            ForEach(namespaceOptions, id: \.self) { namespace in
                Button(namespace) {
                    selectedTagNamespace = namespace
                    newNamespace = ""
                }
            }

            Button("New namespace…") {
                selectedTagNamespace = ""
            }
        } label: {
            HStack {
                Text("Namespace")
                    .font(.subheadline)
                Spacer()
                Text(namespacePickerTitle)
                    .foregroundColor(.secondary)
                Image(systemName: "chevron.up.chevron.down")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
        }
    }

    private var namespacePickerTitle: String {
        if let selectedTagNamespace {
            return selectedTagNamespace.isEmpty ? "New namespace" : selectedTagNamespace
        }
        return "None"
    }

    private func selectedTagChips(_ tags: [TagHierarchySupport.ParsedTag]) -> some View {
        FlexibleTagGrid(items: tags, id: \.name) { tag in
            HStack(spacing: 4) {
                HierarchicalTagChip(name: tag.name)
                Button {
                    self.tags.removeAll { $0 == tag.name }
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }
        }
    }

    private func availableTagChips(_ tags: [TagItem]) -> some View {
        FlexibleTagGrid(items: tags, id: \.id) { tag in
            Button {
                addExistingTag(tag.name)
            } label: {
                HierarchicalTagChip(
                    name: tag.name,
                    isSelected: self.tags.contains {
                        $0.caseInsensitiveCompare(tag.name) == .orderedSame
                    }
                )
            }
            .buttonStyle(.plain)
        }
    }

    private func addExistingTag(_ name: String) {
        guard !tags.contains(where: { $0.caseInsensitiveCompare(name) == .orderedSame }) else {
            return
        }
        tags.append(name)
    }
}
