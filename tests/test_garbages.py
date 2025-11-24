def test_get_garbages(default_api):
    response = default_api.get_garbages()

    assert response.garbages is not None
    assert isinstance(response.garbages, list)

    expected_garbages = {"banana peel", "coffee grounds"}
    actual_garbages = set(response.garbages)
    assert expected_garbages == actual_garbages
