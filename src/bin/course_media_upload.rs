use course_media_multipart::{
    course_delivery::report_delivery,
    infrai_storage::{CompletedPart, InfraiStorage, StorageError},
};
use std::{env, num::ParseIntError};

#[derive(Debug, thiserror::Error)]
enum ServiceError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("invalid command: {0}")]
    Command(String),
    #[error("invalid number: {0}")]
    Number(#[from] ParseIntError),
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[tokio::main]
async fn main() -> Result<(), ServiceError> {
    let args: Vec<String> = env::args().collect();
    let storage = InfraiStorage::from_env()?;

    match args.get(1).map(String::as_str) {
        Some("start") if args.len() == 7 => {
            let bucket = &args[2];
            let course_id = &args[3];
            let media_key = &args[4];
            let part_count: u32 = args[5].parse()?;
            let deadline: u64 = args[6].parse()?;
            storage.create_bucket(bucket).await?;
            let created = storage.create_upload(bucket, media_key).await?;
            let mut urls = Vec::with_capacity(part_count as usize);
            for part_number in 1..=part_count {
                let signed = storage.presign_part(&created.upload_id, part_number).await?;
                urls.push(serde_json::json!({
                    "part_number": part_number,
                    "method": "PUT",
                    "url": signed.url
                }));
            }
            let report = report_delivery(course_id, media_key, 0, part_count as usize, 0, deadline);
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "upload_id": created.upload_id,
                "parts": urls,
                "educator_report": report
            }))?);
        }
        Some("complete") if args.len() == 8 => {
            let course_id = &args[2];
            let media_key = &args[3];
            let upload_id = &args[4];
            let deadline: u64 = args[5].parse()?;
            let now: u64 = args[6].parse()?;
            let parts: Vec<CompletedPart> = serde_json::from_str(&args[7])?;
            storage.complete_upload(upload_id, &parts).await?;
            let report = report_delivery(course_id, media_key, parts.len(), parts.len(), now, deadline);
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        _ => return Err(ServiceError::Command(
            "start <bucket> <course_id> <media_key> <part_count> <deadline_epoch_seconds> | complete <course_id> <media_key> <upload_id> <deadline_epoch_seconds> <now_epoch_seconds> <parts_json>".to_owned(),
        )),
    }
    Ok(())
}

