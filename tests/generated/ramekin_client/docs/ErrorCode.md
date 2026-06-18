# ErrorCode

Machine-readable error code surfaced in every error response.  Clients branch on this instead of parsing the human-readable `error` message or guessing from the HTTP status, so changing the wording of a message is never a contract change. Each code maps to exactly one HTTP status (see [`ErrorCode::status`]).

## Enum

* `NOT_FOUND` (value: `'not_found'`)

* `INVALID_REQUEST` (value: `'invalid_request'`)

* `CONFLICT` (value: `'conflict'`)

* `UNAUTHORIZED` (value: `'unauthorized'`)

* `FORBIDDEN` (value: `'forbidden'`)

* `METHOD_NOT_ALLOWED` (value: `'method_not_allowed'`)

* `PAYLOAD_TOO_LARGE` (value: `'payload_too_large'`)

* `SERVICE_UNAVAILABLE` (value: `'service_unavailable'`)

* `INTERNAL` (value: `'internal'`)

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


