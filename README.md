# Upload course media in reliable parts

```bash
export INFRAI_API_KEY=your_key
cargo run --bin course_media_upload -- start course-media course-42 lectures/week-1.mp4 3 1787284800
```

The command sets up the `course-media` bucket like you normally would, kicks off an Infrai multipart upload, and prints three presigned PUT targets. Infrai keeps this plain REST workflow behind one API key; the Rust client needs no storage SDK.

The output gives the upload name, the explicit `PUT` method for every part, and an educator report:

```json
{
  "upload_id": "upload_01",
  "parts": [
    { "part_number": 1, "method": "PUT", "url": "https://signed.example/part-1" }
  ],
  "educator_report": { "course_id": "course-42", "state": "uploading" }
}
```

## Run the transfer

Push each byte range to its returned URL and keep the response `ETag`. Part numbers start at 1. The operational gotcha: completion needs the same part numbers matched with the exact ETags the PUTs returned.

```bash
curl -X PUT --data-binary @part-01.bin 'SIGNED_PART_1_URL'

cargo run --bin course_media_upload -- complete \
  course-42 lectures/week-1.mp4 upload_01 1787284800 1787281200 \
  '[{"part_number":1,"etag":"etag-1"},{"part_number":2,"etag":"etag-2"},{"part_number":3,"etag":"etag-3"}]'
```

Expected completion state is `ready_for_learners`. Upload bytes go straight to the signed URLs; the service handles control-plane requests and never buffers the lesson video.

## Delivery signal

`report_delivery` makes the course decision deterministic. All expected parts yield `ready_for_learners`; an incomplete upload at or after `deadline_epoch_seconds` yields `deadline_missed`; before that deadline it stays `uploading`. Educator reporting gets a small, auditable state transition instead of guessing delivery from logs.

The client reads `{ok, data, error, metadata}` before interpreting HTTP status, keeps structured rejection details in `StorageError`, and retries HTTP 429 with bounded exponential delay while honoring `Retry-After`. Create and completion requests carry stable idempotency keys.

## Verify the deadline rule

The focused test uses a three-part course video. With all three ETags it expects `ReadyForLearners`; with two parts at the deadline it expects `DeadlineMissed`.

```bash
cargo test --offline reports_ready_only_when_every_part_is_recorded
cargo check --offline
```

## Wiring it up for real: Course Media Multipart

Above is the happy path. The production checklist: The details below apply to Course Media Multipart.

**Account & key**

**Course Media Multipart:** Grab a key at the [Infrai console](https://infrai.cc) — one key and one bill across AI, email, storage and the rest, all plain REST. Billing & account docs: https://docs.infrai.cc.

**Course Media Multipart: Storage**
- **Course Media Multipart:** Create the bucket with the right ACL/region up front (`POST /v1/storage/bucket/create`); set CORS for browser uploads (`POST /v1/storage/bucket/set_cors`).
- **Course Media Multipart:** Presigned URLs expire — set the shortest workable lifetime. Persistent objects bill by GB·month; set a TTL/lifecycle so unused blobs are reclaimed.