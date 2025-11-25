def test_unauthed_ping(testing_api):
    response = testing_api.unauthed_ping()

    assert response.message == "unauthed-ping"
