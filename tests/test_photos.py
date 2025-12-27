import pytest

from ramekin_client.api import PhotosApi
from ramekin_client.exceptions import ApiException


def test_photo_upload_and_download_roundtrip(authed_api_client, test_image):
    """Test that uploading a photo and downloading it returns the same data."""
    client, user_id = authed_api_client
    photos_api = PhotosApi(client)

    # Upload the photo using tuple format (filename, data)
    upload_response = photos_api.upload(file=("test.png", test_image))
    assert upload_response.id is not None
    photo_id = str(upload_response.id)

    # Download the photo
    download_response = photos_api.get_photo_without_preload_content(id=photo_id)
    assert download_response.status == 200
    assert download_response.headers.get("content-type") == "image/png"

    # Verify roundtrip - read the data from the response
    downloaded_data = download_response.data
    assert downloaded_data == test_image


def test_photo_not_found(authed_api_client):
    """Test that requesting a non-existent photo returns 404."""
    client, user_id = authed_api_client
    photos_api = PhotosApi(client)

    # Try to download a non-existent photo
    fake_id = "00000000-0000-0000-0000-000000000000"
    with pytest.raises(ApiException) as exc_info:
        photos_api.get_photo(id=fake_id)

    assert exc_info.value.status == 404


def test_photo_upload_requires_auth(unauthed_api_client):
    """Test that photo upload requires authentication."""
    photos_api = PhotosApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        photos_api.upload(file=("test.png", b"fake"))

    assert exc_info.value.status == 401


def test_photo_download_requires_auth(unauthed_api_client):
    """Test that photo download requires authentication."""
    photos_api = PhotosApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        photos_api.get_photo(id="00000000-0000-0000-0000-000000000000")

    assert exc_info.value.status == 401


def test_photo_thumbnail_endpoint(authed_api_client, test_image):
    """Test that the thumbnail endpoint returns a JPEG thumbnail."""
    client, user_id = authed_api_client
    photos_api = PhotosApi(client)

    # Upload the photo
    upload_response = photos_api.upload(file=("test.png", test_image))
    photo_id = str(upload_response.id)

    # Download the thumbnail
    thumbnail_response = photos_api.get_photo_thumbnail_without_preload_content(
        id=photo_id
    )
    assert thumbnail_response.status == 200
    assert thumbnail_response.headers.get("content-type") == "image/jpeg"

    # Verify the thumbnail is a valid JPEG
    thumbnail_data = thumbnail_response.data
    assert len(thumbnail_data) > 0
    # JPEG files start with FFD8FF
    assert thumbnail_data[:3] == b"\xff\xd8\xff"


def test_photo_thumbnail_not_found(authed_api_client):
    """Test that requesting a thumbnail for a non-existent photo returns 404."""
    client, user_id = authed_api_client
    photos_api = PhotosApi(client)

    fake_id = "00000000-0000-0000-0000-000000000000"
    with pytest.raises(ApiException) as exc_info:
        photos_api.get_photo_thumbnail(id=fake_id)

    assert exc_info.value.status == 404


def test_photo_thumbnail_requires_auth(unauthed_api_client):
    """Test that thumbnail download requires authentication."""
    photos_api = PhotosApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        photos_api.get_photo_thumbnail(id="00000000-0000-0000-0000-000000000000")

    assert exc_info.value.status == 401
