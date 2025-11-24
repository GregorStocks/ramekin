#!/usr/bin/env python3
"""
Basic end-to-end tests for the Ramekin API.
"""

import sys
import os

# Add generated client to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'generated'))

from ramekin_client import ApiClient, Configuration
from ramekin_client.api import DefaultApi


def test_get_garbages():
    """Test that we can fetch garbages from the API."""
    config = Configuration(host=os.environ.get('API_BASE_URL', 'http://server:3000'))

    with ApiClient(config) as client:
        api = DefaultApi(client)
        response = api.get_garbages()

        # Verify the response structure is correct
        assert response.garbages is not None, "Expected garbages in response"
        assert isinstance(response.garbages, list), "Expected garbages to be a list"

        # Check for expected seeded data from migration
        expected_garbages = {"banana peel", "coffee grounds"}
        actual_garbages = set(response.garbages)
        assert expected_garbages == actual_garbages, f"Expected {expected_garbages}, got {actual_garbages}"

        print(f"✓ get_garbages: Found expected garbages: {response.garbages}")


def main():
    """Run all tests."""
    print("=" * 50)
    print("Running Ramekin API Tests")
    print("=" * 50)

    tests = [
        test_get_garbages,
    ]

    passed = 0
    failed = 0

    for test in tests:
        try:
            test()
            passed += 1
        except AssertionError as e:
            print(f"✗ {test.__name__}: {e}")
            failed += 1
        except Exception as e:
            print(f"✗ {test.__name__}: Unexpected error: {e}")
            failed += 1

    print("=" * 50)
    print(f"Results: {passed} passed, {failed} failed")
    print("=" * 50)

    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
