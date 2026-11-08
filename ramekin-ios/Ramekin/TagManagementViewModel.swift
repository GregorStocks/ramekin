import Foundation

/// Injection seams for TagManagementViewModel.
struct TagManagementClient {
    var currentAccountKey: () -> String?
    var listTags: () async throws -> [TagItem]
    /// Renames a tag and returns the name the server stored.
    var renameTag: (_ id: UUID, _ name: String) async throws -> String
    var deleteTag: (_ id: UUID) async throws -> Void

    static let live = TagManagementClient(
        currentAccountKey: { AccountScope.currentAccountKey() },
        listTags: { try await TagsAPI.listAllTags().tags },
        renameTag: { id, name in
            try await TagsAPI.renameTag(id: id, renameTagRequest: RenameTagRequest(name: name)).name
        },
        deleteTag: { try await TagsAPI.deleteTag(id: $0) }
    )
}

/// Loads, renames and deletes tags. Each operation owns its own in-progress
/// and error state, and checks on completion that what it is about to
/// change is still what the user is looking at.
@MainActor
final class TagManagementViewModel: ObservableObject {
    @Published private(set) var tags: [TagItem] = []
    @Published private(set) var isLoading = false
    /// The latest load's failure. Renames and deletes report their own.
    @Published private(set) var loadError: String?

    @Published private(set) var editingTagId: UUID?
    @Published var editName = ""
    @Published private(set) var editError: String?
    /// The tag whose rename is in flight. It can differ from `editingTagId`
    /// when the user starts editing another tag while a save finishes.
    @Published private(set) var savingTagId: UUID?

    /// The tag awaiting delete confirmation.
    @Published var deleteCandidate: TagItem?
    @Published private(set) var deletingTagIds: Set<UUID> = []
    /// Shown as an alert: the tag may be far down the list from the error section.
    @Published var deleteError: String?

    private let client: TagManagementClient
    private var loadGeneration = UUID()

    init(client: TagManagementClient = .live) {
        self.client = client
    }

    var groupedTags: [TagHierarchySupport.TagGroup<TagItem>] {
        TagHierarchySupport.groups(for: tags)
    }

    /// Whether the open editor's tag is saving.
    var isSavingEdit: Bool {
        savingTagId != nil && savingTagId == editingTagId
    }

    func isDeleting(_ tag: TagItem) -> Bool {
        deletingTagIds.contains(tag.id)
    }

    func loadTags() async {
        guard let accountKey = client.currentAccountKey() else {
            tags = []
            isLoading = false
            return
        }
        let generation = UUID()
        loadGeneration = generation
        isLoading = true
        loadError = nil

        do {
            let loaded = try await client.listTags()
            guard loadGeneration == generation else { return }
            isLoading = false
            guard client.currentAccountKey() == accountKey else { return }
            tags = loaded
            TagFilterCache.pruneSelectedTags(validNames: Set(loaded.map(\.name)), accountKey: accountKey)
        } catch {
            guard loadGeneration == generation else { return }
            isLoading = false
            guard !CancellationSupport.isCancellation(error), !Task.isCancelled else { return }
            loadError = APIErrorFormatter.userMessage(from: error, fallback: "Failed to load tags")
        }
    }

    func startEditing(_ tag: TagItem) {
        editingTagId = tag.id
        editName = tag.name
        editError = nil
    }

    func cancelEditing() {
        editingTagId = nil
        editName = ""
        editError = nil
    }

    /// Saves the open editor. Ignored while that tag is already saving, so
    /// pressing Return repeatedly sends one rename.
    func saveEdit() async {
        guard let tag = tags.first(where: { $0.id == editingTagId }) else { return }
        await rename(tag)
    }

    func rename(_ tag: TagItem) async {
        guard savingTagId == nil, let accountKey = client.currentAccountKey() else { return }
        guard let newName = TagManagementSupport.normalizedName(from: editName) else {
            editError = "Tag name cannot be empty"
            return
        }

        savingTagId = tag.id
        editError = nil
        defer { savingTagId = nil }

        do {
            let storedName = try await client.renameTag(tag.id, newName)
            guard client.currentAccountKey() == accountKey else { return }
            tags = TagManagementSupport.renamedTags(tags, id: tag.id, newName: storedName)
            TagFilterCache.renameSelectedTag(from: tag.name, to: storedName, accountKey: accountKey)
            TagFilterCache.notifyTagsDidChange()
            // Only close the editor this save belongs to: the user may have
            // moved on to another tag meanwhile.
            if editingTagId == tag.id {
                cancelEditing()
            }
        } catch {
            guard !CancellationSupport.isCancellation(error), editingTagId == tag.id else { return }
            editError = APIErrorFormatter.userMessage(from: error, fallback: "Failed to rename tag")
        }
    }

    func delete(_ tag: TagItem) async {
        guard !deletingTagIds.contains(tag.id), let accountKey = client.currentAccountKey() else { return }
        deletingTagIds.insert(tag.id)
        defer { deletingTagIds.remove(tag.id) }

        do {
            try await client.deleteTag(tag.id)
            guard client.currentAccountKey() == accountKey else { return }
            tags = TagManagementSupport.removingTag(tags, id: tag.id)
            TagFilterCache.removeSelectedTag(named: tag.name, accountKey: accountKey)
            TagFilterCache.notifyTagsDidChange()
            if editingTagId == tag.id {
                cancelEditing()
            }
        } catch {
            guard !CancellationSupport.isCancellation(error) else { return }
            deleteError = APIErrorFormatter.userMessage(from: error, fallback: "Failed to delete tag")
        }
    }
}
