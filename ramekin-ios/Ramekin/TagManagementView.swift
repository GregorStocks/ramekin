import SwiftUI

struct TagManagementView: View {
    @State private var tags: [TagItem] = []
    @State private var isLoading = false
    @State private var error: String?
    @State private var editingTagId: UUID?
    @State private var editName = ""
    @State private var editError: String?
    @State private var isSaving = false
    @State private var deletingTag: TagItem?

    private var groupedTags: [TagHierarchySupport.TagGroup<TagItem>] {
        TagHierarchySupport.groups(for: tags)
    }

    var body: some View {
        List {
            if let error {
                Section {
                    Text(error)
                        .foregroundColor(.red)
                }
            }

            if isLoading && tags.isEmpty {
                Section {
                    HStack {
                        Spacer()
                        ProgressView("Loading tags...")
                        Spacer()
                    }
                    .padding(.vertical, 12)
                }
            } else if tags.isEmpty {
                Section {
                    VStack(spacing: 12) {
                        Image(systemName: "tag.slash")
                            .font(.system(size: 32))
                            .foregroundColor(.secondary)
                        Text("No tags yet")
                            .font(.headline)
                        Text("Tags are created when you add them to recipes.")
                            .font(.subheadline)
                            .foregroundColor(.secondary)
                            .multilineTextAlignment(.center)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 24)
                }
            } else {
                ForEach(groupedTags) { group in
                    Section(group.title) {
                        ForEach(group.items) { tag in
                            row(for: tag)
                        }
                    }
                }

                Section {
                    EmptyView()
                } footer: {
                    Text("Deleting a tag removes it from recipes that currently use it.")
                }
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle("Manage Tags")
        .navigationBarTitleDisplayMode(.inline)
        .refreshable {
            await loadTags()
        }
        .task {
            await loadTags()
        }
        .alert(
            "Delete Tag",
            isPresented: Binding(
                get: { deletingTag != nil },
                set: { if !$0 { deletingTag = nil } }
            ),
            actions: {
                if let tag = deletingTag {
                    Button("Delete", role: .destructive) {
                        Task {
                            await deleteTag(tag)
                        }
                    }
                }

                Button("Cancel", role: .cancel) {}
            },
            message: {
                if let tag = deletingTag {
                    Text("Remove \"\(tag.name)\" from \(TagManagementSupport.recipeCountText(for: tag.recipeCount))?")
                }
            }
        )
    }
}

private extension TagManagementView {
    @ViewBuilder
    func row(for tag: TagItem) -> some View {
        if editingTagId == tag.id {
            editingRow(for: tag)
        } else {
            tagRow(for: tag)
        }
    }

    func tagRow(for tag: TagItem) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top, spacing: 12) {
                VStack(alignment: .leading, spacing: 4) {
                    HierarchicalTagLabel(name: tag.name, valueFont: .headline, namespaceFont: .subheadline)
                    Text(TagManagementSupport.recipeCountText(for: tag.recipeCount))
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                }

                Spacer()
            }

            HStack(spacing: 12) {
                Button {
                    startEditing(tag)
                } label: {
                    Label("Rename", systemImage: "pencil")
                }
                .buttonStyle(.borderless)

                Button(role: .destructive) {
                    deletingTag = tag
                } label: {
                    Label("Delete", systemImage: "trash")
                }
                .buttonStyle(.borderless)
            }
            .font(.subheadline)
        }
        .padding(.vertical, 6)
    }

    func editingRow(for tag: TagItem) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            TextField("Tag name", text: $editName)
                .textInputAutocapitalization(.never)
                .disableAutocorrection(true)
                .submitLabel(.done)
                .onSubmit {
                    Task {
                        await renameTag(tag)
                    }
                }

            Text(TagManagementSupport.recipeCountText(for: tag.recipeCount))
                .font(.subheadline)
                .foregroundColor(.secondary)

            if let editError {
                Text(editError)
                    .font(.caption)
                    .foregroundColor(.red)
            }

            HStack(spacing: 12) {
                Button {
                    Task {
                        await renameTag(tag)
                    }
                } label: {
                    if isSaving {
                        ProgressView()
                    } else {
                        Text("Save")
                    }
                }
                .buttonStyle(.borderedProminent)
                .disabled(isSaving)

                Button("Cancel") {
                    cancelEditing()
                }
                .buttonStyle(.bordered)
                .disabled(isSaving)
            }
        }
        .padding(.vertical, 6)
    }

    @MainActor
    func startEditing(_ tag: TagItem) {
        editingTagId = tag.id
        editName = tag.name
        editError = nil
    }

    @MainActor
    func cancelEditing() {
        editingTagId = nil
        editName = ""
        editError = nil
    }

    func loadTags() async {
        guard let accountKey = AccountScope.currentAccountKey() else {
            await MainActor.run {
                tags = []
                isLoading = false
            }
            return
        }
        await MainActor.run {
            isLoading = true
            error = nil
        }

        do {
            let response = try await TagsAPI.listAllTags()
            guard AccountScope.currentAccountKey() == accountKey else { return }
            await MainActor.run {
                tags = response.tags
                TagFilterCache.saveAvailableTags(response.tags, accountKey: accountKey)
                TagFilterCache.pruneSelectedTags(
                    validNames: Set(response.tags.map(\.name)),
                    accountKey: accountKey
                )
                isLoading = false
            }
        } catch is CancellationError {
            await MainActor.run {
                isLoading = false
            }
        } catch {
            await MainActor.run {
                self.error = APIErrorFormatter.userMessage(
                    from: error,
                    fallback: "Failed to load tags"
                )
                isLoading = false
            }
        }
    }

    func renameTag(_ tag: TagItem) async {
        guard let accountKey = AccountScope.currentAccountKey() else { return }
        let currentEditName = await MainActor.run { editName }

        guard let newName = TagManagementSupport.normalizedName(from: currentEditName) else {
            await MainActor.run {
                editError = "Tag name cannot be empty"
            }
            return
        }

        await MainActor.run {
            isSaving = true
            editError = nil
        }

        do {
            let response = try await TagsAPI.renameTag(
                id: tag.id,
                renameTagRequest: RenameTagRequest(name: newName)
            )
            guard AccountScope.currentAccountKey() == accountKey else { return }
            await MainActor.run {
                tags = TagManagementSupport.renamedTags(tags, id: tag.id, newName: response.name)
                TagFilterCache.saveAvailableTags(tags, accountKey: accountKey)
                TagFilterCache.renameSelectedTag(
                    from: tag.name,
                    to: response.name,
                    accountKey: accountKey
                )
                TagFilterCache.notifyTagsDidChange()
                error = nil
                isSaving = false
                cancelEditing()
            }
        } catch is CancellationError {
            await MainActor.run {
                isSaving = false
            }
        } catch {
            await MainActor.run {
                editError = APIErrorFormatter.userMessage(
                    from: error,
                    fallback: "Failed to rename tag"
                )
                isSaving = false
            }
        }
    }

    func deleteTag(_ tag: TagItem) async {
        guard let accountKey = AccountScope.currentAccountKey() else { return }
        do {
            try await TagsAPI.deleteTag(id: tag.id)
            guard AccountScope.currentAccountKey() == accountKey else { return }
            await MainActor.run {
                tags = TagManagementSupport.removingTag(tags, id: tag.id)
                TagFilterCache.saveAvailableTags(tags, accountKey: accountKey)
                TagFilterCache.removeSelectedTag(named: tag.name, accountKey: accountKey)
                TagFilterCache.notifyTagsDidChange()
                error = nil
                deletingTag = nil
                if editingTagId == tag.id {
                    cancelEditing()
                }
            }
        } catch is CancellationError {
        } catch {
            await MainActor.run {
                self.error = APIErrorFormatter.userMessage(
                    from: error,
                    fallback: "Failed to delete tag"
                )
                deletingTag = nil
            }
        }
    }
}

#Preview {
    NavigationStack {
        TagManagementView()
    }
}
