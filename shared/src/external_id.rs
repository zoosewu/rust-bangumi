use serde::{Deserialize, Serialize};

/// 一季作品在某外部站台的身分，例如 `bgm/548818`。
///
/// namespace 決定哪個 metadata service 能解析它；id 對其他人而言是不透明的。
/// 線上格式固定為 `"{namespace}/{id}"` 字串。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ExternalId {
    pub namespace: String,
    pub id: String,
}

impl ExternalId {
    pub fn new(namespace: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            id: id.into(),
        }
    }
}

impl std::fmt::Display for ExternalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.namespace, self.id)
    }
}

impl std::str::FromStr for ExternalId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (namespace, id) = s
            .split_once('/')
            .ok_or_else(|| format!("external id missing '/' separator: {s:?}"))?;
        if namespace.is_empty() || id.is_empty() {
            return Err(format!("external id has empty namespace or id: {s:?}"));
        }
        Ok(Self::new(namespace, id))
    }
}

impl TryFrom<String> for ExternalId {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<ExternalId> for String {
    fn from(e: ExternalId) -> String {
        e.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parses_namespace_and_id() {
        let e = ExternalId::from_str("bgm/548818").unwrap();
        assert_eq!(e.namespace, "bgm");
        assert_eq!(e.id, "548818");
    }

    #[test]
    fn round_trips_through_display() {
        let e = ExternalId::from_str("mikan/3822").unwrap();
        assert_eq!(e.to_string(), "mikan/3822");
    }

    #[test]
    fn keeps_slashes_inside_the_id() {
        // 只在第一個 '/' 切開，讓 tmdb/tv/1234 這種階層 id 仍可表達
        let e = ExternalId::from_str("tmdb/tv/1234").unwrap();
        assert_eq!(e.namespace, "tmdb");
        assert_eq!(e.id, "tv/1234");
    }

    #[test]
    fn rejects_input_without_separator() {
        assert!(ExternalId::from_str("bgm").is_err());
    }

    #[test]
    fn rejects_empty_namespace_or_id() {
        assert!(ExternalId::from_str("/548818").is_err());
        assert!(ExternalId::from_str("bgm/").is_err());
    }

    #[test]
    fn serialises_as_a_plain_string() {
        let e = ExternalId::new("bgm", "548818");
        assert_eq!(serde_json::to_string(&e).unwrap(), r#""bgm/548818""#);
        let back: ExternalId = serde_json::from_str(r#""bgm/548818""#).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn rejects_malformed_string_during_deserialisation() {
        assert!(serde_json::from_str::<ExternalId>(r#""bgm""#).is_err());
    }

    #[test]
    fn raw_items_from_an_older_fetcher_have_no_external_ids() {
        let item: crate::RawAnimeItem = serde_json::from_str(
            r#"{"title":"[X] Y [01]","description":null,"download_url":"u","pub_date":null}"#,
        )
        .unwrap();
        assert!(item.external_ids.is_empty());
    }

    #[test]
    fn raw_item_external_ids_travel_as_strings() {
        let item: crate::RawAnimeItem = serde_json::from_str(
            r#"{"title":"t","description":null,"download_url":"u","pub_date":null,
                "external_ids":["mikan/3822","bgm/548818"]}"#,
        )
        .unwrap();
        assert_eq!(
            item.external_ids,
            vec![
                ExternalId::new("mikan", "3822"),
                ExternalId::new("bgm", "548818")
            ]
        );
    }

    #[test]
    fn capabilities_from_an_older_service_declare_no_namespaces() {
        let caps: crate::Capabilities = serde_json::from_str("{}").unwrap();
        assert!(caps.namespaces.is_empty());
    }
}
