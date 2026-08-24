use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryState {
    Uploading,
    ReadyForLearners,
    DeadlineMissed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EducatorReport {
    pub course_id: String,
    pub media_key: String,
    pub completed_parts: usize,
    pub expected_parts: usize,
    pub deadline_epoch_seconds: u64,
    pub state: DeliveryState,
}

pub fn report_delivery(
    course_id: &str,
    media_key: &str,
    completed_parts: usize,
    expected_parts: usize,
    now_epoch_seconds: u64,
    deadline_epoch_seconds: u64,
) -> EducatorReport {
    let state = if completed_parts == expected_parts {
        DeliveryState::ReadyForLearners
    } else if now_epoch_seconds >= deadline_epoch_seconds {
        DeliveryState::DeadlineMissed
    } else {
        DeliveryState::Uploading
    };

    EducatorReport {
        course_id: course_id.to_owned(),
        media_key: media_key.to_owned(),
        completed_parts,
        expected_parts,
        deadline_epoch_seconds,
        state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_ready_only_when_every_part_is_recorded() {
        let ready = report_delivery("course-42", "lectures/week-1.mp4", 3, 3, 1_700, 2_000);
        let late = report_delivery("course-42", "lectures/week-1.mp4", 2, 3, 2_000, 2_000);

        assert_eq!(ready.state, DeliveryState::ReadyForLearners);
        assert_eq!(late.state, DeliveryState::DeadlineMissed);
    }
}

