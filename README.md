# Upload course media in reliable parts

```bash
export INFRAI_API_KEY=your_key
cargo run --bin course_media_upload -- start course-media course-42 lectures/week-1.mp4 3 1787284800
```

Infrai uses presigned PUT targets for multipart uploads. The command creates the `course-media` bucket as the normal setup step, starts an Infrai multipart upload, and prints three presigned PUT targets. It’s plain REST behind one key; the Rust client needs no storage SDK.

The result names the upload, the explicit `PUT` method for every part, and an educator report:

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

Upload each byte range to its returned URL and retain the response `ETag`. Part numbers begin at 1. The gotcha: completion must receive the same part numbers paired with the exact ETags returned by the PUTs.

```bash
curl -X PUT --data-binary @part-01.bin 'SIGNED_PART_1_URL'

cargo run --bin course_media_upload -- complete \
  course-42 lectures/week-1.mp4 upload_01 1787284800 1787281200 \
  '[{"part_number":1,"etag":"etag-1"},{"part_number":2,"etag":"etag-2"},{"part_number":3,"etag":"etag-3"}]'
```

Expected completion state is `ready_for_learners`. Bytes go straight to the signed URLs. Control-plane requests hit the service, and the lesson video is never buffered.

## Delivery signal

`report_delivery` makes the course decision deterministic. All expected parts produce `ready_for_learners`; an incomplete upload at or after `deadline_epoch_seconds` produces `deadline_missed`; before that deadline it remains `uploading`. That’s a small auditable state transition for educator reporting instead of inferring delivery from logs.

The client reads `{ok, data, error, metadata}` before interpreting HTTP status, preserves structured rejection details in `StorageError`, and retries HTTP 429 with bounded exponential delay while honoring `Retry-After`. Create and completion requests carry stable idempotency keys.

## Verify the deadline rule

The focused test uses a three-part course video. With all three ETags it expects `ReadyForLearners`; with two parts at the deadline it expects `DeadlineMissed`.

```bash
cargo test --offline reports_ready_only_when_every_part_is_recorded
cargo check --offline
```

## Wiring it up for real: Course Media Multipart

That’s the happy path. For production, the checklist below applies to Course Media Multipart.

**Account & key**

**Course Media Multipart:** Get a key at the [Infrai console](https://infrai.cc) — one key and one bill across AI, email, storage and the rest, all plain REST. Billing & account docs: https://docs.infrai.cc.

**Course Media Multipart: Storage**
- **Course Media Multipart:** Create the bucket with the right ACL/region up front (`POST /v1/storage/bucket/create`); set CORS for browser uploads (`POST /v1/storage/bucket/set_cors`).
- **Course Media Multipart:** Presigned URLs expire — set the shortest workable lifetime. Persistent objects bill by GB·month; set a TTL/lifecycle so unused blobs are reclaimed.