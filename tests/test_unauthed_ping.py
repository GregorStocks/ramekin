def test_unauthed_ping(default_api):
    response = default_api.unauthed_ping()

    assert response.message == "unauthed-ping"
