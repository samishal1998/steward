use std::{collections::BTreeSet, fs::File, io::Read, path::Path};

use reqwest::Url;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;

use crate::{
    error::{Diagnostic, Result},
    model::{Config, Manifest},
};

pub const MAX_CONFIG: usize = 1024 * 1024;
pub const MAX_RESPONSE: usize = 8 * 1024 * 1024;
pub const MAX_RECORDS: usize = 10_000;

pub fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let file = File::open(path)
        .map_err(|_| Diagnostic::new("input.unreadable", "Cannot open the requested input file"))?;
    if !file
        .metadata()
        .map_err(|_| Diagnostic::new("input.unreadable", "Cannot inspect the input file"))?
        .is_file()
    {
        return Err(Diagnostic::new(
            "input.invalid",
            "Input must be a regular file",
        ));
    }
    let mut bytes = Vec::new();
    file.take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| Diagnostic::new("input.unreadable", "Cannot read the input file"))?;
    if bytes.len() > limit {
        return Err(Diagnostic::new(
            "input.too_large",
            format!("Input exceeds {limit} bytes"),
        ));
    }
    Ok(bytes)
}

pub fn load<T: DeserializeOwned>(path: &Path, limit: usize) -> Result<T> {
    let bytes = read_bounded(path, limit)?;
    let value = if path.extension().is_some_and(|e| e == "json") {
        strict_json(&bytes)?
    } else {
        let options = serde_saphyr::options! {
            strict_booleans: true,
            reject_unsupported_tags: true,
            merge_keys: serde_saphyr::MergeKeyPolicy::Error,
            budget: serde_saphyr::budget! {
                max_depth: 64, max_documents: 1, max_aliases: 0, max_anchors: 0,
                max_events: 100_000, max_nodes: 50_000, max_total_scalar_bytes: limit,
            },
        };
        serde_saphyr::from_slice_with_options::<Value>(&bytes, options).map_err(|_| {
            Diagnostic::new(
                "input.invalid_yaml",
                "Invalid YAML: check types, duplicate keys, tags, nesting, and document count",
            )
        })?
    };
    serde_json::from_value(value).map_err(|_| Diagnostic::new("input.schema", "Input does not match schema version 1; check required fields, types, and unknown fields"))
}

pub fn load_config(path: &Path) -> Result<Config> {
    let config: Config = load(path, MAX_CONFIG)?;
    version(&config.schema_version)?;
    if config.proxies.is_empty() || config.proxies.len() > 16 {
        return Err(Diagnostic::new(
            "config.bindings",
            "Specify between 1 and 16 explicit proxy bindings",
        ));
    }
    let mut ids = BTreeSet::new();
    for binding in &config.proxies {
        resource_id(&binding.id)?;
        if !ids.insert(&binding.id) {
            return Err(Diagnostic::new(
                "config.duplicate_id",
                "Proxy IDs must be unique",
            ));
        }
        validate_endpoint(&binding.endpoint)?;
        if let Some(auth) = &binding.auth_file
            && (!Path::new(auth).is_absolute() || auth.chars().any(char::is_control))
        {
            return Err(Diagnostic::new(
                "config.auth_file",
                "Authorization file must be an absolute path",
            ));
        }
    }
    Ok(config)
}

pub fn load_manifest(path: &Path) -> Result<Manifest> {
    let mut manifest: Manifest = load(path, MAX_CONFIG)?;
    version(&manifest.schema_version)?;
    if !manifest.services.is_empty() {
        return Err(Diagnostic::new(
            "capability.unsupported",
            "General service planning is a later milestone; no proxy was contacted",
        ));
    }
    if manifest.routes.len() > 256 {
        return Err(Diagnostic::new(
            "input.too_large",
            "At most 256 route specifications per plan",
        ));
    }
    let mut ids = BTreeSet::new();
    for route in &mut manifest.routes {
        resource_id(&route.id)?;
        resource_id(&route.proxy)?;
        if !ids.insert(&route.id) {
            return Err(Diagnostic::new(
                "config.duplicate_id",
                "Route IDs must be unique",
            ));
        }
        route.hostname = exact_host(&route.hostname)?;
        let upstream = http_url(&route.upstream.url)?;
        if upstream.fragment().is_some()
            || upstream.query().is_some()
            || !matches!(upstream.path(), "" | "/")
        {
            return Err(Diagnostic::new(
                "route.upstream",
                "Upstream must be one HTTP(S) origin without query, fragment, or path rewriting",
            ));
        }
    }
    Ok(manifest)
}

pub fn version(value: &str) -> Result<()> {
    if value != "1" {
        return Err(Diagnostic::new(
            "schema.unsupported",
            "Only schema_version \"1\" is supported",
        ));
    }
    Ok(())
}

pub fn resource_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err(Diagnostic::new(
            "input.id",
            "IDs must be 1–64 ASCII letters, digits, hyphens, or underscores",
        ));
    }
    Ok(())
}

pub fn exact_host(host: &str) -> Result<String> {
    let host = host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase();
    if host.len() > 253
        || host.is_empty()
        || !host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
    {
        return Err(Diagnostic::new(
            "route.hostname",
            "Use a concrete ASCII/punycode DNS hostname, not a wildcard, URL, or IP authority",
        ));
    }
    if host.parse::<std::net::IpAddr>().is_ok() {
        return Err(Diagnostic::new(
            "route.hostname",
            "IP-literal routes are outside the first writable subset",
        ));
    }
    Ok(host)
}

pub fn http_url(value: &str) -> Result<Url> {
    if value.chars().any(char::is_control) {
        return Err(Diagnostic::new(
            "config.endpoint",
            "URL must not contain control characters",
        ));
    }
    let url =
        Url::parse(value).map_err(|_| Diagnostic::new("config.endpoint", "Invalid HTTP(S) URL"))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(Diagnostic::new(
            "config.endpoint",
            "Use HTTP(S) without URL credentials",
        ));
    }
    Ok(url)
}

pub fn validate_endpoint(value: &str) -> Result<()> {
    if value.len() > 2048 || value.chars().any(char::is_control) {
        return Err(Diagnostic::new(
            "config.endpoint",
            "Invalid endpoint length or characters",
        ));
    }
    if let Some(socket) = value.strip_prefix("unix://") {
        if !Path::new(socket).is_absolute() || socket.contains(['?', '#']) {
            return Err(Diagnostic::new(
                "config.endpoint",
                "Use unix:///absolute/socket",
            ));
        }
        return Ok(());
    }
    let url = http_url(value)?;
    if url.query().is_some() || url.fragment().is_some() {
        return Err(Diagnostic::new(
            "config.endpoint",
            "API base endpoint must not contain query or fragment",
        ));
    }
    Ok(())
}

// serde_json::Value ordinarily accepts duplicate object keys. Reject them recursively.
struct UniqueValue(Value);
impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = UniqueValue;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("JSON with unique object keys")
            }
            fn visit_bool<E: serde::de::Error>(
                self,
                v: bool,
            ) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<Self::Value, E> {
                serde_json::Number::from_f64(v)
                    .map(|n| UniqueValue(n.into()))
                    .ok_or_else(|| E::custom("nonfinite number"))
            }
            fn visit_str<E: serde::de::Error>(
                self,
                v: &str,
            ) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_string<E: serde::de::Error>(
                self,
                v: String,
            ) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut out = Vec::new();
                while let Some(v) = seq.next_element::<UniqueValue>()? {
                    out.push(v.0);
                }
                Ok(UniqueValue(out.into()))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut out = serde_json::Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if out.contains_key(&key) {
                        return Err(serde::de::Error::custom("duplicate key"));
                    }
                    out.insert(key, map.next_value::<UniqueValue>()?.0);
                }
                Ok(UniqueValue(out.into()))
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

pub fn strict_json(bytes: &[u8]) -> Result<Value> {
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let value = UniqueValue::deserialize(&mut decoder).map_err(|_| {
        Diagnostic::new(
            "input.invalid_json",
            "Invalid JSON, duplicate keys, or excessive nesting",
        )
    })?;
    decoder
        .end()
        .map_err(|_| Diagnostic::new("input.invalid_json", "Expected one JSON document"))?;
    Ok(value.0)
}
