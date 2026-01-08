# Data Directory

This directory contains test data and generated files.

## test-urls.json

A generated list of top recipe sites and their URLs, used for testing the recipe scraping pipeline.

### Generating the file

```bash
# Generate with defaults (50 sites, 10 URLs per site)
make generate-test-urls

# Or with custom options
cd cli && cargo run -- generate-test-urls --num-sites 50 --urls-per-site 20

# Merge with existing file (incremental updates)
cd cli && cargo run -- generate-test-urls --merge
```

### CLI Options

| Option | Default | Description |
|--------|---------|-------------|
| `--output`, `-o` | `data/test-urls.json` | Output file path |
| `--num-sites` | 50 | Number of sites to include |
| `--urls-per-site` | 20 | Target number of URLs per site |
| `--merge` | false | Merge with existing file instead of replacing |

### How it works

1. **Site rankings**: Fetches top food blogs from [detailed.com/food-blogs](https://detailed.com/food-blogs/), with a fallback to a hardcoded list of ~48 known recipe sites.

2. **URL discovery**: For each site, tries (in order):
   - Parse `sitemap.xml` (handles both urlset and sitemap indexes)
   - Check `robots.txt` for sitemap location
   - Scrape homepage for recipe links

3. **Recipe URL filtering**: URLs are filtered to likely recipe pages based on patterns like `/recipe/`, `/recipes/`, date-based paths (`/2025/01/`), etc.

4. **Rate limiting**: 1 second delay between sites, 200ms between sub-sitemap fetches.

### Output format

```json
{
  "generated_at": "2026-01-08T00:41:07.348374+00:00",
  "config": {
    "num_sites": 50,
    "urls_per_site": 20
  },
  "sites": [
    {
      "domain": "smittenkitchen.com",
      "rank": 10,
      "urls": [
        "https://smittenkitchen.com/2025/12/winter-cabbage-salad/",
        "..."
      ],
      "source": "sitemap"
    },
    {
      "domain": "food52.com",
      "rank": 2,
      "urls": [],
      "error": "Could not find recipe URLs from sitemap or homepage",
      "source": "failed"
    }
  ]
}
```

### Source values

- `sitemap`: URLs found via sitemap.xml
- `homepage`: URLs found by scraping the homepage
- `merged`: URLs from both a previous run and current run (when using `--merge`)
- `failed`: Could not find URLs from any source

### Merge mode

With `--merge`, the tool:
- Reads the existing file first
- Skips sites that already have enough URLs
- Unions new URLs with existing ones (deduped)
- Preserves sites from previous runs even if rankings changed

This lets you incrementally grow the dataset over time without re-scraping everything.
