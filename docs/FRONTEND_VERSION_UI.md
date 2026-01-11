# Frontend Version UI Implementation Plan

This document outlines what needs to be implemented in the frontend to complete the recipe versioning feature.

## Backend APIs Available

The following new APIs are available for the frontend to use:

### GET /api/recipes/{id}/versions
Lists all versions of a recipe.

**Response:**
```json
{
  "versions": [
    {
      "id": "uuid",
      "title": "Recipe Title",
      "version_source": "user" | "scrape" | "enrich",
      "created_at": "2026-01-11T12:00:00Z",
      "is_current": true
    }
  ]
}
```

Versions are returned newest-first (descending by created_at).

### GET /api/recipes/{id}?version_id={version_id}
Get a specific version of a recipe instead of the current version.

### POST /api/enrich
Stateless AI enrichment - takes a recipe object, returns enriched version without modifying the database.

**Request:**
```json
{
  "title": "...",
  "description": "...",
  "ingredients": [{"item": "..."}],
  "instructions": "...",
  "tags": ["..."],
  "servings": "...",
  "prep_time": "...",
  "cook_time": "...",
  "total_time": "...",
  "difficulty": "...",
  "notes": "..."
}
```

**Response:** Same structure with enriched values.

## UI Components to Implement

### 1. Version History Panel (ViewRecipePage)

Add a collapsible section or sidebar showing version history:

```
Version History
---------------
[Current] Jan 11, 2026 2:30 PM - user
          "Updated ingredient amounts"

Jan 11, 2026 1:15 PM - enrich
          "AI enrichment"
          [View] [Revert]

Jan 11, 2026 1:14 PM - scrape
          "Initial import"
          [View] [Revert]
```

**Features:**
- Show version_source with appropriate icon/badge (user, scrape, enrich)
- Show created_at timestamp
- Show title (to see what changed)
- "View" button to preview that version
- "Revert" button to restore that version (creates a new version with that content)

### 2. Version Diff Viewer

When viewing a non-current version, show a diff against the current version:

- Side-by-side or inline diff view
- Highlight changes in:
  - Title
  - Description
  - Ingredients (additions/removals/changes)
  - Instructions
  - Tags
  - Metadata (prep_time, cook_time, etc.)

**Implementation options:**
- Use a diff library like `diff` or `diff-match-patch`
- Simple approach: Show "Current" vs "This Version" side-by-side

### 3. Revert Confirmation Dialog

When user clicks "Revert":

```
Revert to this version?
-----------------------
This will create a new version with the content from
"Jan 11, 2026 1:14 PM - scrape".

The current version will be preserved in history.

[Cancel] [Revert]
```

### 4. Manual Enrich Button

Add an "Enrich with AI" button on the recipe view/edit page:

1. Call POST /api/enrich with current recipe content
2. Show diff preview of proposed changes
3. User can accept/reject (accept = PUT /api/recipes/{id} with enriched content)

```
AI Suggestions
--------------
[Show diff of enriched vs current]

[Reject] [Apply Changes]
```

### 5. Version Source Badges

Display version source on recipe cards and detail view:

- "user" - User icon
- "scrape" - Globe/link icon
- "enrich" - Sparkles/AI icon

## TypeScript Types (already generated)

The OpenAPI client has been regenerated with new types:

```typescript
// ramekin-ui/generated-client/models/VersionListResponse.ts
// ramekin-ui/generated-client/models/VersionSummary.ts
// ramekin-ui/generated-client/models/EnrichRequest.ts
// ramekin-ui/generated-client/models/EnrichResponse.ts

// RecipeResponse now includes:
interface RecipeResponse {
  // ... existing fields ...
  version_id: string;
  version_source: string;
}
```

## API Client Methods (already generated)

```typescript
// RecipesApi
recipesApi.listVersions(id: string): Promise<VersionListResponse>
recipesApi.getRecipe(id: string, versionId?: string): Promise<RecipeResponse>

// EnrichApi
enrichApi.enrichRecipe(enrichRequest: EnrichRequest): Promise<EnrichResponse>
```

## Implementation Order

1. **Version History Panel** - Display version history on ViewRecipePage
2. **Version Viewing** - Add ability to view specific versions
3. **Revert Feature** - Allow reverting to previous versions
4. **Version Source Badges** - Visual indicators for version sources
5. **Enrich Button** - Manual AI enrichment with diff preview
6. **Diff Viewer** - Client-side diff display (optional, can start simple)

## Notes

- All version operations are non-destructive (reverts create new versions)
- The `version_source` field tracks origin: "user", "scrape", "enrich"
- Auto-enrichment runs during scraping if `ANTHROPIC_API_KEY` is set
- The enrich endpoint requires auth but is stateless (no DB changes)
