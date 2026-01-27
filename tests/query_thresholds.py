"""
Per-endpoint database query count thresholds.

Each key is "{METHOD} {path_pattern}" where path_pattern uses
the OpenAPI route pattern (e.g., /api/recipes/{id}).

Values are (min, max) tuples:
- min: Expected minimum queries. If actual < min, the endpoint got more efficient
       and we should update the threshold (prevents threshold inflation over time).
- max: Maximum acceptable queries. If actual > max, it likely indicates
       an N+1 query problem or regression.

To update thresholds after making efficiency improvements:
1. Run tests with QUERY_THRESHOLD_DISCOVERY=1 to see actual counts
2. Update the thresholds here to match the new baseline
"""

# (min_queries, max_queries) tuples
# Updated based on actual measurements 2026-01-27
# Small buffer added to handle slight variability between runs
QUERY_THRESHOLDS: dict[str, tuple[int, int]] = {
    # Auth endpoints
    "POST /api/auth/signup": (3, 4),
    "POST /api/auth/login": (3, 4),
    # Recipe endpoints
    "GET /api/recipes": (5, 7),
    "POST /api/recipes": (13, 16),  # varies 14-15
    "GET /api/recipes/{id}": (5, 7),
    "PUT /api/recipes/{id}": (15, 17),
    "DELETE /api/recipes/{id}": (5, 7),
    "GET /api/recipes/{id}/versions": (6, 8),
    "GET /api/recipes/export": (5, 12),
    "GET /api/recipes/{id}/export": (5, 10),
    "POST /api/recipes/{id}/rescrape": (8, 22),
    # Tag endpoints
    "GET /api/tags": (5, 7),
    "POST /api/tags": (4, 10),
    "DELETE /api/tags/{id}": (4, 10),
    # Photo endpoints
    "POST /api/photos/upload": (4, 10),
    "GET /api/photos/{id}": (4, 7),
    "GET /api/photos/{id}/thumbnail": (4, 7),
    # Scrape endpoints
    "POST /api/scrape": (4, 12),
    "GET /api/scrape/{id}": (4, 10),
    "POST /api/scrape/{id}/retry": (4, 12),
    "POST /api/scrape/capture": (4, 18),
    # Enrich
    "POST /api/enrich": (0, 6),
    # Test endpoints
    "GET /api/test/ping": (4, 6),
    "GET /api/test/unauthed-ping": (0, 0),
}

# Default thresholds for endpoints not explicitly listed
# Using wide range initially - tighten as we discover actual counts
DEFAULT_MIN = 0
DEFAULT_MAX = 20


def get_thresholds(endpoint: str) -> tuple[int, int]:
    """Get (min, max) thresholds for an endpoint."""
    return QUERY_THRESHOLDS.get(endpoint, (DEFAULT_MIN, DEFAULT_MAX))
