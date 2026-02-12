use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::features::user::models::Permission;

#[derive(Debug, Deserialize, Clone)]
pub struct Jwks {
    pub keys: [Jwk; 2],
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
pub struct Jwk {
    pub kid: String,
    pub n: String,
    pub e: String,
    pub kty: String,
    pub alg: String,
    #[serde(rename(deserialize = "use"))]
    pub use_: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum StringOrVec {
    String(String),
    Vec(Vec<String>),
}

impl From<StringOrVec> for Vec<String> {
    fn from(value: StringOrVec) -> Self {
        match value {
            StringOrVec::String(s) => vec![s],
            StringOrVec::Vec(v) => v,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    gty: Option<String>,
    #[serde(deserialize_with = "deserialize_aud")]
    aud: Vec<String>,
    azp: String,
    exp: i32,
    iat: i32,
    iss: String,
    pub scope: String,
    pub sub: String,
    pub permissions: Option<HashSet<Permission>>,
}

fn deserialize_aud<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    StringOrVec::deserialize(deserializer).map(Into::into)
}

impl Claims {
    pub fn empty() -> Self {
        Self {
            gty: None,
            aud: Vec::new(),
            azp: String::new(),
            exp: 0,
            iat: 0,
            iss: String::new(),
            scope: String::new(),
            sub: String::from("guest"),
            permissions: None,
        }
    }

    pub fn is_machine(&self) -> bool {
        self.gty == Some("client-credentials".to_string())
    }

    pub fn auth0_id(&self) -> &str {
        &self.sub
    }

    pub fn missing_permission<I>(&self, required: I) -> Option<HashSet<Permission>>
    where
        I: IntoIterator<Item = Permission>,
    {
        let required_iter = required.into_iter();
        let permissions = match &self.permissions {
            None => return Some(required_iter.collect()),
            Some(perm) => perm,
        };

        let missing: HashSet<Permission> = required_iter
            .filter(|p: &Permission| !permissions.contains(p))
            .collect();

        (!missing.is_empty()).then_some(missing)
    }
}
