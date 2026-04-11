/// 用于处理 SurrealDB Thing ID 的序列化/反序列化辅助模块

use serde::{Deserialize, Deserializer, Serializer};
use serde::de::IntoDeserializer;
use chrono::{DateTime, Utc};

/// 处理 SurrealDB 的 Thing ID 格式 (例如: "tag:xxxxx")
pub mod thing_id {
    use super::*;
    
    fn extract_key(value: &serde_json::Value) -> Option<String> {
        match value {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Number(n) => Some(n.to_string()),
            serde_json::Value::Bool(b) => Some(b.to_string()),
            serde_json::Value::Object(map) => {
                if let Some(table) = map.get("table").and_then(|v| v.as_str()) {
                    if let Some(key_val) = map.get("key").or_else(|| map.get("id")) {
                        if let Some(key) = extract_key(key_val) {
                            return Some(format!("{}:{}", table, key));
                        }
                    }
                }
                if let Some(tb) = map.get("tb").and_then(|v| v.as_str()) {
                    if let Some(id_val) = map.get("id").or_else(|| map.get("key")) {
                        if let Some(key) = extract_key(id_val) {
                            return Some(format!("{}:{}", tb, key));
                        }
                    }
                }
                if let Some(thing_val) = map.get("Thing") {
                    return extract_key(thing_val);
                }
                if let Some(record_val) = map.get("RecordId") {
                    return extract_key(record_val);
                }
                if let Some(v) = map.get("String") {
                    return extract_key(v);
                }
                if let Some(v) = map.get("Strand") {
                    return extract_key(v);
                }
                if let Some(v) = map.get("value") {
                    return extract_key(v);
                }
                if let Some(v) = map.get("id") {
                    return extract_key(v);
                }
                if let Some(v) = map.get("key") {
                    return extract_key(v);
                }
                if let Some(v) = map.get("Int") {
                    return extract_key(v);
                }
                if let Some(v) = map.get("Float") {
                    return extract_key(v);
                }
                if let Some(v) = map.get("Number") {
                    return extract_key(v);
                }
                if let Some(v) = map.get("Uuid") {
                    return extract_key(v);
                }
                None
            }
            _ => None,
        }
    }
    
    pub fn serialize<S>(id: &str, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(id)
    }
    
    pub fn deserialize<'de, D>(deserializer: D) -> Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;

        match value {
            serde_json::Value::String(s) => Ok(s),
            serde_json::Value::Object(mut map) => {
                // Wrapped shape: {"Thing": {"tb":"article","id": ...}}
                if let Some(thing) = map.remove("Thing") {
                    if let serde_json::Value::Object(mut thing_map) = thing {
                        let tb = thing_map
                            .remove("tb")
                            .and_then(|v| v.as_str().map(|s| s.to_string()))
                            .unwrap_or_default();
                        if !tb.is_empty() {
                            if let Some(id_val) = thing_map.remove("id").or_else(|| thing_map.remove("key")) {
                                if let Some(key) = extract_key(&id_val) {
                                    return Ok(format!("{}:{}", tb, key));
                                }
                            }
                        }
                    }
                }

                // Direct shape: {"tb":"article","id": ...}
                if let Some(tb) = map.get("tb").and_then(|v| v.as_str()) {
                    if let Some(id_val) = map.get("id").or_else(|| map.get("key")) {
                        if let Some(key) = extract_key(id_val) {
                            if key.contains(':') {
                                return Ok(key);
                            }
                            return Ok(format!("{}:{}", tb, key));
                        }
                    }
                }

                // RecordId-ish shape: {"table":"article","key": ...}
                if let Some(table) = map.get("table").and_then(|v| v.as_str()) {
                    if let Some(key_val) = map.get("key").or_else(|| map.get("id")) {
                        if let Some(key) = extract_key(key_val) {
                            if key.contains(':') {
                                return Ok(key);
                            }
                            return Ok(format!("{}:{}", table, key));
                        }
                    }
                }

                // Alternate wrapper shape: {"RecordId": {...}}
                if let Some(record_val) = map.get("RecordId") {
                    if let Some(key) = extract_key(record_val) {
                        return Ok(key);
                    }
                }

                // Pass-through scalar wrappers when no table context.
                if let Some(key) = map
                    .get("String")
                    .or_else(|| map.get("Strand"))
                    .or_else(|| map.get("value"))
                    .and_then(extract_key)
                {
                    Ok(key)
                } else {
                    // Last-resort fallback to keep deserialization resilient across
                    // Surreal/soulcore response shape variants.
                    Ok(serde_json::Value::Object(map).to_string())
                }
            }
            _ => Err(serde::de::Error::custom("invalid record id type")),
        }
    }
}

/// 处理可选的 SurrealDB Thing ID
pub mod thing_id_option {
    use super::*;

    pub fn serialize<S>(id: &Option<String>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match id {
            Some(v) => thing_id::serialize(v, serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(None),
            serde_json::Value::Object(ref map) if map.contains_key("None") => Ok(None),
            other => {
                let parsed = thing_id::deserialize(other.into_deserializer())
                    .map_err(|e| serde::de::Error::custom(e.to_string()))?;
                Ok(Some(parsed))
            }
        }
    }
}

/// 处理 SurrealDB 的 DateTime 格式
pub mod surrealdb_datetime {
    use super::*;
    use chrono::format::Fixed;
    use chrono::TimeZone;
    use serde::de::{self, Unexpected};
    
    pub fn serialize<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // 使用 SurrealDB 期望的格式
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("datetime", 1)?;
        state.serialize_field("datetime", &dt.to_rfc3339())?;
        state.end()
    }
    
    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum DateTimeValue {
            String(String),
            Object { datetime: String },
        }
        
        match DateTimeValue::deserialize(deserializer)? {
            DateTimeValue::String(s) => {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|_| de::Error::invalid_value(Unexpected::Str(&s), &"RFC3339 datetime"))
            }
            DateTimeValue::Object { datetime } => {
                DateTime::parse_from_rfc3339(&datetime)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|_| de::Error::invalid_value(Unexpected::Str(&datetime), &"RFC3339 datetime"))
            }
        }
    }
}

/// 处理可选的 SurrealDB DateTime
pub mod surrealdb_datetime_option {
    use super::*;
    
    pub fn serialize<S>(dt: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match dt {
            Some(dt) => surrealdb_datetime::serialize(dt, serializer),
            None => serializer.serialize_none(),
        }
    }
    
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum OptionalDateTime {
            Some(#[serde(with = "surrealdb_datetime")] DateTime<Utc>),
            None,
        }
        
        match OptionalDateTime::deserialize(deserializer)? {
            OptionalDateTime::Some(dt) => Ok(Some(dt)),
            OptionalDateTime::None => Ok(None),
        }
    }
}

pub mod loose_i64 {
    use super::*;
    use serde::de::Error as _;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<i64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Number(n) => n
                .as_i64()
                .ok_or_else(|| D::Error::custom("invalid numeric value")),
            serde_json::Value::String(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
                    Ok(0)
                } else {
                    trimmed
                        .parse::<i64>()
                        .map_err(|_| D::Error::custom(format!("invalid i64 string: {trimmed}")))
                }
            }
            serde_json::Value::Null => Ok(0),
            serde_json::Value::Object(map) if map.contains_key("None") => Ok(0),
            other => Err(D::Error::custom(format!("invalid i64 value: {other}"))),
        }
    }
}

pub mod loose_datetime_now {
    use super::*;
    use serde::de::Error as _;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Utc::now()),
            serde_json::Value::String(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
                    Ok(Utc::now())
                } else {
                    DateTime::parse_from_rfc3339(trimmed)
                        .map(|dt| dt.with_timezone(&Utc))
                        .map_err(|_| D::Error::custom(format!("invalid RFC3339 datetime: {trimmed}")))
                }
            }
            serde_json::Value::Object(map) if map.contains_key("None") => Ok(Utc::now()),
            other => {
                let fallback = other.clone();
                surrealdb_datetime::deserialize(other.into_deserializer())
                    .map_err(|e| D::Error::custom(e.to_string()))
                    .or_else(|_| Err(D::Error::custom(format!("invalid datetime value: {fallback}"))))
            }
        }
    }
}
