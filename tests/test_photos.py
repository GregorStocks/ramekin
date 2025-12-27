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
