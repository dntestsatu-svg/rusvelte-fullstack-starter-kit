use serde::de::{Error as DeError, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct PaginationParams {
    #[serde(default = "default_page", deserialize_with = "deserialize_u32")]
    pub page: u32,
    #[serde(
        default = "default_per_page",
        alias = "limit",
        deserialize_with = "deserialize_u32"
    )]
    pub per_page: u32,
    #[serde(default, deserialize_with = "deserialize_optional_u64")]
    pub offset: Option<u64>,
}

impl PaginationParams {
    pub fn normalize(self) -> Self {
        let page = if self.page < 1 { 1 } else { self.page };
        let per_page = if self.per_page < 1 {
            20
        } else if self.per_page > 100 {
            100
        } else {
            self.per_page
        };

        let offset = self
            .offset
            .unwrap_or_else(|| ((page.saturating_sub(1)) as u64) * (per_page as u64));
        let page = ((offset / per_page as u64) as u32) + 1;

        Self {
            page,
            per_page,
            offset: Some(offset),
        }
    }

    pub fn offset(&self) -> u64 {
        self.offset
            .unwrap_or_else(|| ((self.page.saturating_sub(1)) as u64) * (self.per_page as u64))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total_count: u64,
    pub total_pages: u32,
    pub page: u32,
    pub per_page: u32,
}

impl<T> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, total_count: u64, params: PaginationParams) -> Self {
        let params = params.normalize();
        let total_pages = (total_count as f64 / params.per_page as f64).ceil() as u32;

        Self {
            data,
            total_count,
            total_pages,
            page: params.page,
            per_page: params.per_page,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_normalization() {
        let p = PaginationParams {
            page: 0,
            per_page: 0,
            offset: None,
        }
        .normalize();
        assert_eq!(p.page, 1);
        assert_eq!(p.per_page, 20);
        assert_eq!(p.offset(), 0);

        let p2 = PaginationParams {
            page: 5,
            per_page: 500,
            offset: None,
        }
        .normalize();
        assert_eq!(p2.page, 5);
        assert_eq!(p2.per_page, 100);

        let p3 = PaginationParams {
            page: 1,
            per_page: 10,
            offset: Some(20),
        }
        .normalize();
        assert_eq!(p3.page, 3);
        assert_eq!(p3.offset(), 20);
    }

    #[test]
    fn test_pagination_offset() {
        let p = PaginationParams {
            page: 1,
            per_page: 20,
            offset: None,
        };
        assert_eq!(p.offset(), 0);

        let p2 = PaginationParams {
            page: 2,
            per_page: 20,
            offset: None,
        };
        assert_eq!(p2.offset(), 20);
    }
}

const fn default_page() -> u32 {
    1
}

const fn default_per_page() -> u32 {
    20
}

fn deserialize_u32<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    struct U32Visitor;

    impl<'de> Visitor<'de> for U32Visitor {
        type Value = u32;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a u32 or a numeric string")
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: DeError,
        {
            u32::try_from(value).map_err(|_| E::custom("value is out of range for u32"))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: DeError,
        {
            if value < 0 {
                return Err(E::custom("value must be positive"));
            }

            self.visit_u64(value as u64)
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: DeError,
        {
            value
                .parse::<u32>()
                .map_err(|_| E::custom("invalid numeric string"))
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: DeError,
        {
            self.visit_str(&value)
        }
    }

    deserializer.deserialize_any(U32Visitor)
}

fn deserialize_optional_u64<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    struct OptionalU64Visitor;

    impl<'de> Visitor<'de> for OptionalU64Visitor {
        type Value = Option<u64>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an optional u64 or numeric string")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: DeError,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: DeError,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserialize_u64(deserializer).map(Some)
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: DeError,
        {
            Ok(Some(value))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: DeError,
        {
            if value < 0 {
                return Err(E::custom("value must be positive"));
            }

            Ok(Some(value as u64))
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: DeError,
        {
            if value.trim().is_empty() {
                return Ok(None);
            }

            value
                .parse::<u64>()
                .map(Some)
                .map_err(|_| E::custom("invalid numeric string"))
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: DeError,
        {
            self.visit_str(&value)
        }
    }

    deserializer.deserialize_option(OptionalU64Visitor)
}

fn deserialize_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    struct U64Visitor;

    impl<'de> Visitor<'de> for U64Visitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a u64 or a numeric string")
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: DeError,
        {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: DeError,
        {
            if value < 0 {
                return Err(E::custom("value must be positive"));
            }

            Ok(value as u64)
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: DeError,
        {
            value
                .parse::<u64>()
                .map_err(|_| E::custom("invalid numeric string"))
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: DeError,
        {
            self.visit_str(&value)
        }
    }

    deserializer.deserialize_any(U64Visitor)
}
