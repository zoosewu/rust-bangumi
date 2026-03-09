/// Custom serializers for NaiveDateTime that append "Z" (UTC marker)
/// so JavaScript correctly treats them as UTC and converts to local timezone.
pub mod naive_datetime_utc {
    use chrono::NaiveDateTime;
    use serde::Serializer;

    pub fn serialize<S>(dt: &NaiveDateTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}Z", dt.format("%Y-%m-%dT%H:%M:%S")))
    }
}

pub mod naive_datetime_utc_opt {
    use chrono::NaiveDateTime;
    use serde::Serializer;

    pub fn serialize<S>(dt: &Option<NaiveDateTime>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match dt {
            Some(dt) => {
                serializer.serialize_str(&format!("{}Z", dt.format("%Y-%m-%dT%H:%M:%S")))
            }
            None => serializer.serialize_none(),
        }
    }
}

/// Format a NaiveDateTime as UTC ISO 8601 string with Z suffix.
pub fn format_utc(dt: &chrono::NaiveDateTime) -> String {
    format!("{}Z", dt.format("%Y-%m-%dT%H:%M:%S"))
}

/// Format an optional NaiveDateTime as UTC ISO 8601 string with Z suffix.
pub fn format_utc_opt(dt: &Option<chrono::NaiveDateTime>) -> Option<String> {
    dt.as_ref().map(format_utc)
}
