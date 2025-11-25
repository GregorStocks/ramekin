import base64

import pytest

from ramekin_client.api import PhotosApi
from ramekin_client.exceptions import ApiException


def test_photo_upload_and_download_roundtrip(authed_api_client):
    """Test that uploading a photo and downloading it returns the same data."""
    client, user_id = authed_api_client
    photos_api = PhotosApi(client)

    # Create test image data (a simple 1x1 PNG)
    test_image_data = bytes(
        [
            0x89,
            0x50,
            0x4E,
            0x47,
            0x0D,
            0x0A,
            0x1A,
            0x0A,  # PNG signature
            0x00,
            0x00,
            0x00,
            0x0D,
            0x49,
            0x48,
            0x44,
            0x52,  # IHDR chunk
            0x00,
            0x00,
            0x00,
            0x01,
            0x00,
            0x00,
            0x00,
            0x01,  # 1x1
            0x08,
            0x02,
            0x00,
            0x00,
            0x00,
            0x90,
            0x77,
            0x53,
            0xDE,  # 8-bit RGB
            0x00,
            0x00,
            0x00,
            0x0C,
            0x49,
            0x44,
            0x41,
            0x54,  # IDAT chunk
            0x08,
            0xD7,
            0x63,
            0xF8,
            0xFF,
            0xFF,
            0x3F,
            0x00,
            0x05,
            0xFE,
            0x02,
            0xFE,
            0xDC,
            0xCC,
            0x59,
            0xE7,
            0x00,
            0x00,
            0x00,
            0x00,
            0x49,
            0x45,
            0x4E,
            0x44,  # IEND chunk
            0xAE,
            0x42,
            0x60,
            0x82,
        ]
    )

    # Upload the photo using tuple format (filename, data)
    upload_response = photos_api.upload(file=("test.png", test_image_data))
    assert upload_response.id is not None
    photo_id = str(upload_response.id)

    # Download the photo
    download_response = photos_api.get_photo_without_preload_content(id=photo_id)
    assert download_response.status == 200
    assert download_response.headers.get("content-type") == "image/png"

    # Verify roundtrip - read the data from the response
    downloaded_data = download_response.data
    assert downloaded_data == test_image_data


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


def test_list_photos_empty(authed_api_client):
    """Test listing photos when user has no photos."""
    client, user_id = authed_api_client
    photos_api = PhotosApi(client)

    response = photos_api.list_photos()
    assert response.photos == []


def test_list_photos_returns_uploaded_photos(authed_api_client):
    """Test that list_photos returns photos the user has uploaded."""
    client, user_id = authed_api_client
    photos_api = PhotosApi(client)

    # Create test image data (a simple 1x1 PNG)
    test_image_data = bytes(
        [
            0x89,
            0x50,
            0x4E,
            0x47,
            0x0D,
            0x0A,
            0x1A,
            0x0A,
            0x00,
            0x00,
            0x00,
            0x0D,
            0x49,
            0x48,
            0x44,
            0x52,
            0x00,
            0x00,
            0x00,
            0x01,
            0x00,
            0x00,
            0x00,
            0x01,
            0x08,
            0x02,
            0x00,
            0x00,
            0x00,
            0x90,
            0x77,
            0x53,
            0xDE,
            0x00,
            0x00,
            0x00,
            0x0C,
            0x49,
            0x44,
            0x41,
            0x54,
            0x08,
            0xD7,
            0x63,
            0xF8,
            0xFF,
            0xFF,
            0x3F,
            0x00,
            0x05,
            0xFE,
            0x02,
            0xFE,
            0xDC,
            0xCC,
            0x59,
            0xE7,
            0x00,
            0x00,
            0x00,
            0x00,
            0x49,
            0x45,
            0x4E,
            0x44,
            0xAE,
            0x42,
            0x60,
            0x82,
        ]
    )

    # Upload two photos
    upload1 = photos_api.upload(file=("test1.png", test_image_data))
    upload2 = photos_api.upload(file=("test2.png", test_image_data))

    # List photos
    response = photos_api.list_photos()
    assert len(response.photos) == 2

    # Check that both uploaded photos are in the list
    photo_ids = {str(p.id) for p in response.photos}
    assert str(upload1.id) in photo_ids
    assert str(upload2.id) in photo_ids

    # Verify each photo has required fields
    for photo in response.photos:
        assert photo.id is not None
        assert photo.content_type == "image/png"
        assert photo.created_at is not None
        assert photo.thumbnail is not None
        # Thumbnail should be valid base64
        thumbnail_bytes = base64.b64decode(photo.thumbnail)
        assert len(thumbnail_bytes) > 0
        # Thumbnail should be a JPEG (starts with FFD8FF)
        assert thumbnail_bytes[:3] == b"\xff\xd8\xff"


def test_list_photos_requires_auth(unauthed_api_client):
    """Test that listing photos requires authentication."""
    photos_api = PhotosApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        photos_api.list_photos()

    assert exc_info.value.status == 401


def test_list_photos_only_returns_own_photos(
    authed_api_client, second_authed_api_client
):
    """Test that users can only see their own photos."""
    client1, user1_id = authed_api_client
    client2, user2_id = second_authed_api_client
    photos_api1 = PhotosApi(client1)
    photos_api2 = PhotosApi(client2)

    # Create test image
    test_image_data = bytes(
        [
            0x89,
            0x50,
            0x4E,
            0x47,
            0x0D,
            0x0A,
            0x1A,
            0x0A,
            0x00,
            0x00,
            0x00,
            0x0D,
            0x49,
            0x48,
            0x44,
            0x52,
            0x00,
            0x00,
            0x00,
            0x01,
            0x00,
            0x00,
            0x00,
            0x01,
            0x08,
            0x02,
            0x00,
            0x00,
            0x00,
            0x90,
            0x77,
            0x53,
            0xDE,
            0x00,
            0x00,
            0x00,
            0x0C,
            0x49,
            0x44,
            0x41,
            0x54,
            0x08,
            0xD7,
            0x63,
            0xF8,
            0xFF,
            0xFF,
            0x3F,
            0x00,
            0x05,
            0xFE,
            0x02,
            0xFE,
            0xDC,
            0xCC,
            0x59,
            0xE7,
            0x00,
            0x00,
            0x00,
            0x00,
            0x49,
            0x45,
            0x4E,
            0x44,
            0xAE,
            0x42,
            0x60,
            0x82,
        ]
    )

    # User 1 uploads a photo
    photos_api1.upload(file=("user1.png", test_image_data))

    # User 2 uploads a photo
    photos_api2.upload(file=("user2.png", test_image_data))

    # User 1 should only see their own photo
    user1_photos = photos_api1.list_photos()
    assert len(user1_photos.photos) == 1

    # User 2 should only see their own photo
    user2_photos = photos_api2.list_photos()
    assert len(user2_photos.photos) == 1

    # The photos should be different
    assert user1_photos.photos[0].id != user2_photos.photos[0].id
