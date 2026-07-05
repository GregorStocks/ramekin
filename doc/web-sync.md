# Web Sync Strategy

The web cookbook intentionally stays server-backed and lazy-loaded.

`GET /api/recipes` is paginated and supports the full web search/filter/sort
surface. The cookbook currently loads one page at a time and fetches more on
scroll. By contrast, `GET /api/recipes/sync` is unpaginated: a first sync
returns every active recipe summary for the user. Using it for normal cookbook
loads would change web from lazy loading to a full local recipe-summary cache.

Web should not adopt iOS local recipe search unless the sync endpoint grows a
paginated or scoped mode that preserves the current load shape and search
semantics. iOS local search is intentionally cache-limited; web search includes
server-side matching and ranking across fields that are not present in recipe
summaries.

The web shopping list does use `POST /api/shopping-list/sync`. The shopping
list already loads as a complete list, so syncing it into localStorage gives a
warm cache and basic offline read tolerance without changing cookbook paging or
search behavior.
