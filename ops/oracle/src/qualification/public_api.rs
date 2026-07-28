use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use thiserror::Error;

use super::model::PublicApiKind;

mod identity;
mod policy;
mod reexports;
mod schema;

use identity::{canonical_value_digest, resolved_path_id, resolved_path_name};
use policy::is_evidence_only_stab_core_export;
use reexports::ExternalReexport;
pub(super) use reexports::{ResolvedExternalReexport, resolve_external_reexports};
use schema::{
    direct_kind, first_key, inner, is_doc_hidden, is_public, item, join_path, json_id,
    rustc_host_target, rustdoc_path, source_span, validate_api_path,
};

const MAX_RUSTDOC_JSON_BYTES: usize = 32 << 20;
const MAX_PUBLIC_API_ITEMS: usize = 16_384;
const MAX_PUBLIC_API_PATH_BYTES: usize = 1_024;
const MAX_TRAVERSAL_STEPS: usize = 65_536;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ExtractedPublicApiItem {
    pub(super) crate_name: String,
    pub(super) path: String,
    pub(super) kind: PublicApiKind,
    pub(super) source_path: PathBuf,
    pub(super) source_line: u32,
    pub(super) owner_path: String,
    pub(super) evidence_crate_name: String,
    pub(super) evidence_owner_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RustdocInventory {
    pub(super) crate_name: String,
    pub(super) format_version: u64,
    pub(super) items: Vec<ExtractedPublicApiItem>,
    visible_reexports: BTreeMap<String, String>,
    external_reexports: Vec<ExternalReexport>,
}

#[derive(Debug, Error)]
pub(crate) enum PublicApiError {
    #[error("failed to run rustdoc JSON for package {package}: {reason}")]
    RustdocProcess { package: String, reason: Box<str> },

    #[error(
        "rustdoc JSON for package {package} exited with {status}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    )]
    RustdocFailed {
        package: String,
        status: String,
        stdout: Box<str>,
        stderr: Box<str>,
    },

    #[error("failed to read rustdoc JSON {path}: {reason}")]
    ReadRustdoc { path: PathBuf, reason: Box<str> },

    #[error("failed to parse rustdoc JSON {path}: {source}")]
    ParseRustdoc {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("rustdoc JSON is missing or has invalid field {0:?}")]
    InvalidField(&'static str),

    #[error("rustdoc JSON references missing item id {0}")]
    MissingItem(String),

    #[error(
        "public API re-export {alias_path:?} targets unresolved external Stab path {canonical_path:?}"
    )]
    UnresolvedExternalReexport {
        alias_path: String,
        canonical_path: String,
    },

    #[error("failed to determine the pinned Rust host target: {0}")]
    HostTarget(Box<str>),

    #[error("public API traversal exceeded the {limit}-step limit")]
    TraversalLimit { limit: usize },

    #[error("public API item {path:?} has no stable source span")]
    MissingSpan { path: String },

    #[error("public API item path {path:?} is invalid")]
    InvalidPath { path: String },

    #[error("public API inventory has {actual} items; limit is {limit}")]
    TooManyItems { actual: usize, limit: usize },

    #[error("rustdoc produced duplicate public API identity {path:?} ({kind:?})")]
    DuplicateIdentity { path: String, kind: PublicApiKind },

    #[error("default-feature rustdoc exposed evidence-only API {0:?}")]
    EvidenceOnlyExport(String),
}

pub(super) fn generate_rustdoc_inventory(
    root: &Path,
    package: &str,
    crate_name: &str,
) -> Result<RustdocInventory, PublicApiError> {
    let host_target = rustc_host_target(root)?;
    let target_guard = tempfile::Builder::new()
        .prefix(&format!("{crate_name}-"))
        .tempdir()
        .map_err(|source| PublicApiError::RustdocProcess {
            package: package.to_string(),
            reason: format!("failed to create fresh rustdoc target: {source}").into_boxed_str(),
        })?;
    let target_dir = target_guard.path();
    let rustdoc_path = target_dir
        .join(&host_target)
        .join("doc")
        .join(format!("{}.json", crate_name.replace('-', "_")));
    let args = [
        OsString::from("rustdoc"),
        OsString::from("-q"),
        OsString::from("-p"),
        OsString::from(package),
        OsString::from("--lib"),
        OsString::from("--target-dir"),
        target_dir.as_os_str().to_owned(),
        OsString::from("--target"),
        OsString::from(&host_target),
        OsString::from("--"),
        OsString::from("-Z"),
        OsString::from("unstable-options"),
        OsString::from("--output-format"),
        OsString::from("json"),
    ];
    let monitored_rustdoc = crate::safe_file::SafeFileLocation::path(rustdoc_path.clone());
    let output = crate::process::run_process_monitoring_files(
        Path::new("cargo"),
        args,
        &[],
        Some(root),
        &[monitored_rustdoc],
        u64::try_from(MAX_RUSTDOC_JSON_BYTES).unwrap_or(u64::MAX),
    )
    .map_err(|source| PublicApiError::RustdocProcess {
        package: package.to_string(),
        reason: source.to_string().into_boxed_str(),
    })?;
    if !output.success() {
        return Err(PublicApiError::RustdocFailed {
            package: package.to_string(),
            status: crate::process::display_status(output.status),
            stdout: output.stdout.render_for_diagnostics().into_boxed_str(),
            stderr: output.stderr.render_for_diagnostics().into_boxed_str(),
        });
    }
    let bytes = crate::safe_file::read_regular_file_bounded(&rustdoc_path, MAX_RUSTDOC_JSON_BYTES)
        .map_err(|source| PublicApiError::ReadRustdoc {
            path: rustdoc_path.clone(),
            reason: source.to_string().into_boxed_str(),
        })?;
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|source| PublicApiError::ParseRustdoc {
            path: rustdoc_path,
            source,
        })?;
    extract_rustdoc_inventory(&value, crate_name)
}

fn extract_rustdoc_inventory(
    value: &Value,
    expected_crate_name: &str,
) -> Result<RustdocInventory, PublicApiError> {
    let format_version = value
        .get("format_version")
        .and_then(Value::as_u64)
        .ok_or(PublicApiError::InvalidField("format_version"))?;
    if value.get("includes_private").and_then(Value::as_bool) != Some(false) {
        return Err(PublicApiError::InvalidField("includes_private"));
    }
    let root_id = value
        .get("root")
        .and_then(Value::as_u64)
        .ok_or(PublicApiError::InvalidField("root"))?
        .to_string();
    let index = value
        .get("index")
        .and_then(Value::as_object)
        .ok_or(PublicApiError::InvalidField("index"))?;
    let empty_paths = Map::new();
    let paths = value
        .get("paths")
        .and_then(Value::as_object)
        .unwrap_or(&empty_paths);
    if expected_crate_name == "stab_core" {
        for entry in index.values() {
            if let Some(name) = entry.get("name").and_then(Value::as_str)
                && is_evidence_only_stab_core_export(name)
            {
                return Err(PublicApiError::EvidenceOnlyExport(name.to_string()));
            }
        }
    }
    let root = item(index, &root_id)?;
    let crate_name = root
        .get("name")
        .and_then(Value::as_str)
        .ok_or(PublicApiError::InvalidField("root.name"))?;
    if crate_name != expected_crate_name {
        return Err(PublicApiError::InvalidField("root.name mismatch"));
    }

    let mut collector = ApiCollector {
        crate_name,
        index,
        paths,
        items: BTreeMap::new(),
        visible_reexports: BTreeMap::new(),
        external_reexports: BTreeSet::new(),
        visited_modules: BTreeSet::new(),
        visited_globs: BTreeSet::new(),
        traversal_steps: 0,
    };
    collector.collect_module(&root_id, crate_name)?;
    if collector.items.len() > MAX_PUBLIC_API_ITEMS {
        return Err(PublicApiError::TooManyItems {
            actual: collector.items.len(),
            limit: MAX_PUBLIC_API_ITEMS,
        });
    }
    Ok(RustdocInventory {
        crate_name: crate_name.to_string(),
        format_version,
        items: collector.items.into_values().collect(),
        visible_reexports: collector.visible_reexports,
        external_reexports: collector.external_reexports.into_iter().collect(),
    })
}

struct ApiCollector<'a> {
    crate_name: &'a str,
    index: &'a Map<String, Value>,
    paths: &'a Map<String, Value>,
    items: BTreeMap<(String, PublicApiKind), ExtractedPublicApiItem>,
    visible_reexports: BTreeMap<String, String>,
    external_reexports: BTreeSet<ExternalReexport>,
    visited_modules: BTreeSet<(String, String)>,
    visited_globs: BTreeSet<(String, String)>,
    traversal_steps: usize,
}

impl ApiCollector<'_> {
    fn collect_module(&mut self, module_id: &str, module_path: &str) -> Result<(), PublicApiError> {
        if !self
            .visited_modules
            .insert((module_id.to_string(), module_path.to_string()))
        {
            return Ok(());
        }
        let module = item(self.index, module_id)?;
        let item_ids = module
            .get("inner")
            .and_then(|inner| inner.get("module"))
            .and_then(|module| module.get("items"))
            .and_then(Value::as_array)
            .ok_or(PublicApiError::InvalidField("module.items"))?;
        for item_id in item_ids {
            let item_id = json_id(item_id)?;
            self.collect_reachable_item(&item_id, module_path, None)?;
        }
        Ok(())
    }

    fn collect_reachable_item(
        &mut self,
        item_id: &str,
        parent_path: &str,
        alias_name: Option<&str>,
    ) -> Result<(), PublicApiError> {
        self.traversal_steps = self.traversal_steps.saturating_add(1);
        if self.traversal_steps > MAX_TRAVERSAL_STEPS {
            return Err(PublicApiError::TraversalLimit {
                limit: MAX_TRAVERSAL_STEPS,
            });
        }
        let value = item(self.index, item_id)?;
        if is_doc_hidden(value) || !is_public(value) {
            return Ok(());
        }
        let inner = inner(value)?;
        if let Some(use_value) = inner.get("use") {
            return self.collect_use(value, use_value, parent_path);
        }
        let kind_name = first_key(inner)?;
        let Some(kind) = direct_kind(kind_name) else {
            return Ok(());
        };
        let name = alias_name
            .or_else(|| value.get("name").and_then(Value::as_str))
            .ok_or(PublicApiError::InvalidField("public item name"))?;
        let path = join_path(parent_path, name)?;
        if kind == PublicApiKind::Module {
            self.collect_module(item_id, &path)?;
        } else {
            self.insert_item(value, path.clone(), kind, path.clone())?;
            self.collect_children_and_impls(value, &path)?;
        }
        Ok(())
    }

    fn collect_use(
        &mut self,
        use_item: &Value,
        use_value: &Value,
        parent_path: &str,
    ) -> Result<(), PublicApiError> {
        let target_id = match use_value.get("id") {
            Some(Value::Number(number)) => number.to_string(),
            Some(Value::String(value)) => value.clone(),
            Some(Value::Null) | None => return Ok(()),
            Some(_) => return Err(PublicApiError::InvalidField("use.id")),
        };
        let is_glob = use_value
            .get("is_glob")
            .and_then(Value::as_bool)
            .ok_or(PublicApiError::InvalidField("use.is_glob"))?;
        if is_glob {
            return self.collect_glob_use(use_item, &target_id, parent_path);
        }
        let name = use_value
            .get("name")
            .and_then(Value::as_str)
            .ok_or(PublicApiError::InvalidField("use.name"))?;
        let Some(target) = self.index.get(&target_id) else {
            let alias_path = join_path(parent_path, name)?;
            return self.collect_external_reexport(use_item, &target_id, alias_path);
        };
        if is_doc_hidden(target) {
            return Ok(());
        }
        let kind_name = first_key(inner(target)?)?;
        let Some(kind) = direct_kind(kind_name) else {
            return Ok(());
        };
        let path = join_path(parent_path, name)?;
        if kind == PublicApiKind::Module {
            self.collect_module(&target_id, &path)?;
        } else {
            self.record_visible_reexport(&target_id, &path)?;
            self.insert_item_with_span(target, use_item, path.clone(), kind, path.clone())?;
            self.collect_children_and_impls(target, &path)?;
        }
        Ok(())
    }

    fn record_visible_reexport(
        &mut self,
        target_id: &str,
        visible_path: &str,
    ) -> Result<(), PublicApiError> {
        let Some(canonical_path) = rustdoc_path(self.paths, target_id)? else {
            return Ok(());
        };
        if canonical_path == visible_path {
            return Ok(());
        }
        match self.visible_reexports.entry(canonical_path) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(visible_path.to_string());
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let current = entry.get();
                let candidate = visible_path;
                let current_depth = current.matches("::").count();
                let candidate_depth = candidate.matches("::").count();
                if (candidate_depth, candidate) < (current_depth, current.as_str()) {
                    entry.insert(candidate.to_string());
                }
            }
        }
        Ok(())
    }

    fn collect_glob_use(
        &mut self,
        use_item: &Value,
        target_id: &str,
        parent_path: &str,
    ) -> Result<(), PublicApiError> {
        if !self
            .visited_globs
            .insert((target_id.to_string(), parent_path.to_string()))
        {
            return Ok(());
        }
        let Some(target) = self.index.get(target_id) else {
            return self.collect_external_reexport(use_item, target_id, parent_path.to_string());
        };
        let item_ids = target
            .get("inner")
            .and_then(|inner| inner.get("module"))
            .and_then(|module| module.get("items"))
            .and_then(Value::as_array)
            .ok_or(PublicApiError::InvalidField("glob module.items"))?;
        for item_id in item_ids {
            let item_id = json_id(item_id)?;
            self.collect_reachable_item(&item_id, parent_path, None)?;
        }
        Ok(())
    }

    fn collect_external_reexport(
        &mut self,
        span_item: &Value,
        target_id: &str,
        alias_path: String,
    ) -> Result<(), PublicApiError> {
        let path_entry = self
            .paths
            .get(target_id)
            .ok_or_else(|| PublicApiError::MissingItem(target_id.to_string()))?;
        let components = path_entry
            .get("path")
            .and_then(Value::as_array)
            .ok_or(PublicApiError::InvalidField("paths.path"))?
            .iter()
            .map(|component| {
                component
                    .as_str()
                    .ok_or(PublicApiError::InvalidField("paths.path component"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let Some(canonical_crate_name) = components.first().copied() else {
            return Err(PublicApiError::InvalidField("paths.path"));
        };
        if !canonical_crate_name.starts_with("stab_") {
            return Ok(());
        }
        let canonical_path = components.join("::");
        validate_api_path(&canonical_path)?;
        validate_api_path(&alias_path)?;
        let (source_path, source_line) =
            source_span(span_item).ok_or_else(|| PublicApiError::MissingSpan {
                path: alias_path.clone(),
            })?;
        self.external_reexports.insert(ExternalReexport {
            canonical_crate_name: canonical_crate_name.to_string(),
            canonical_path,
            alias_path,
            source_path,
            source_line,
        });
        Ok(())
    }

    fn collect_children_and_impls(
        &mut self,
        value: &Value,
        owner_path: &str,
    ) -> Result<(), PublicApiError> {
        let inner = inner(value)?;
        let kind = first_key(inner)?;
        let body = inner
            .get(kind)
            .ok_or(PublicApiError::InvalidField("item inner body"))?;
        match kind {
            "enum" => self.collect_enum_variants(body, owner_path)?,
            "struct" => self.collect_struct_fields(body, owner_path)?,
            "union" => self.collect_named_ids(body, "fields", owner_path, true)?,
            "trait" => self.collect_trait_items(body, owner_path)?,
            _ => {}
        }
        if let Some(impl_ids) = body.get("impls").and_then(Value::as_array) {
            for impl_id in impl_ids {
                self.collect_impl(&json_id(impl_id)?, owner_path, value)?;
            }
        }
        Ok(())
    }

    fn collect_named_ids(
        &mut self,
        body: &Value,
        field: &'static str,
        owner_path: &str,
        require_public: bool,
    ) -> Result<(), PublicApiError> {
        let ids = body
            .get(field)
            .and_then(Value::as_array)
            .ok_or(PublicApiError::InvalidField(field))?;
        for id in ids {
            let value = item(self.index, &json_id(id)?)?;
            if is_doc_hidden(value) || (require_public && !is_public(value)) {
                continue;
            }
            let name = value
                .get("name")
                .and_then(Value::as_str)
                .ok_or(PublicApiError::InvalidField("child name"))?;
            let path = join_path(owner_path, name)?;
            let kind = if field == "variants" {
                PublicApiKind::Variant
            } else {
                PublicApiKind::Field
            };
            self.insert_item(value, path, kind, owner_path.to_string())?;
        }
        Ok(())
    }

    fn collect_enum_variants(
        &mut self,
        body: &Value,
        owner_path: &str,
    ) -> Result<(), PublicApiError> {
        let ids = body
            .get("variants")
            .and_then(Value::as_array)
            .ok_or(PublicApiError::InvalidField("variants"))?;
        for id in ids {
            let variant = item(self.index, &json_id(id)?)?;
            if is_doc_hidden(variant) {
                continue;
            }
            let name = variant
                .get("name")
                .and_then(Value::as_str)
                .ok_or(PublicApiError::InvalidField("variant name"))?;
            let variant_path = join_path(owner_path, name)?;
            self.insert_item(
                variant,
                variant_path.clone(),
                PublicApiKind::Variant,
                owner_path.to_string(),
            )?;
            self.collect_variant_fields(variant, &variant_path, owner_path)?;
        }
        Ok(())
    }

    fn collect_variant_fields(
        &mut self,
        variant: &Value,
        variant_path: &str,
        owner_path: &str,
    ) -> Result<(), PublicApiError> {
        let kind = inner(variant)?
            .get("variant")
            .and_then(|body| body.get("kind"))
            .ok_or(PublicApiError::InvalidField("variant.kind"))?;
        let field_ids = if let Some(tuple) = kind.get("tuple").and_then(Value::as_array) {
            Some(tuple)
        } else {
            kind.get("struct")
                .and_then(|value| value.get("fields"))
                .and_then(Value::as_array)
        };
        let Some(field_ids) = field_ids else {
            return Ok(());
        };
        for id in field_ids {
            let field = item(self.index, &json_id(id)?)?;
            if is_doc_hidden(field) {
                continue;
            }
            let name = field
                .get("name")
                .and_then(Value::as_str)
                .ok_or(PublicApiError::InvalidField("variant field name"))?;
            let path = join_path(variant_path, name)?;
            self.insert_item(field, path, PublicApiKind::Field, owner_path.to_string())?;
        }
        Ok(())
    }

    fn collect_struct_fields(
        &mut self,
        body: &Value,
        owner_path: &str,
    ) -> Result<(), PublicApiError> {
        let Some(kind) = body.get("kind") else {
            return Err(PublicApiError::InvalidField("struct.kind"));
        };
        let field_ids = kind
            .get("plain")
            .and_then(|plain| plain.get("fields"))
            .or_else(|| kind.get("tuple"))
            .and_then(Value::as_array);
        let Some(field_ids) = field_ids else {
            return Ok(());
        };
        for id in field_ids {
            if id.is_null() {
                continue;
            }
            let value = item(self.index, &json_id(id)?)?;
            if !is_public(value) || is_doc_hidden(value) {
                continue;
            }
            let name = value
                .get("name")
                .and_then(Value::as_str)
                .ok_or(PublicApiError::InvalidField("struct field name"))?;
            let path = join_path(owner_path, name)?;
            self.insert_item(value, path, PublicApiKind::Field, owner_path.to_string())?;
        }
        Ok(())
    }

    fn collect_trait_items(
        &mut self,
        body: &Value,
        owner_path: &str,
    ) -> Result<(), PublicApiError> {
        let ids = body
            .get("items")
            .and_then(Value::as_array)
            .ok_or(PublicApiError::InvalidField("trait.items"))?;
        for id in ids {
            let value = item(self.index, &json_id(id)?)?;
            if is_doc_hidden(value) {
                continue;
            }
            let name = value
                .get("name")
                .and_then(Value::as_str)
                .ok_or(PublicApiError::InvalidField("trait item name"))?;
            let path = join_path(owner_path, name)?;
            let kind = match first_key(inner(value)?)? {
                "function" => PublicApiKind::TraitMethod,
                "assoc_const" => PublicApiKind::Constant,
                "assoc_type" => PublicApiKind::TypeAlias,
                _ => continue,
            };
            self.insert_item(value, path.clone(), kind, path)?;
        }
        Ok(())
    }

    fn collect_impl(
        &mut self,
        impl_id: &str,
        owner_path: &str,
        owner_value: &Value,
    ) -> Result<(), PublicApiError> {
        let value = item(self.index, impl_id)?;
        let Some(body) = inner(value)?.get("impl") else {
            return Ok(());
        };
        if body
            .get("is_synthetic")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || !matches!(body.get("blanket_impl"), None | Some(Value::Null))
            || body.get("is_negative").and_then(Value::as_bool) == Some(true)
        {
            return Ok(());
        }
        let owner_id = json_id(
            owner_value
                .get("id")
                .ok_or(PublicApiError::InvalidField("owner.id"))?,
        )?;
        let implementing_type = body
            .get("for")
            .ok_or(PublicApiError::InvalidField("impl.for"))?;
        if resolved_path_id(implementing_type)?.as_deref() != Some(owner_id.as_str()) {
            return Ok(());
        }
        if let Some(trait_value) = body.get("trait")
            && !trait_value.is_null()
        {
            let trait_path = resolved_path_name(trait_value, self.paths)?;
            let self_digest = canonical_value_digest(implementing_type);
            let path = format!("{owner_path} as {trait_path} for@{self_digest}");
            validate_api_path(&path)?;
            self.insert_item_with_span(
                value,
                owner_value,
                path,
                PublicApiKind::TraitImpl,
                owner_path.to_string(),
            )?;
            return Ok(());
        }
        let ids = body
            .get("items")
            .and_then(Value::as_array)
            .ok_or(PublicApiError::InvalidField("impl.items"))?;
        for id in ids {
            let value = item(self.index, &json_id(id)?)?;
            if !is_public(value) || is_doc_hidden(value) {
                continue;
            }
            let name = value
                .get("name")
                .and_then(Value::as_str)
                .ok_or(PublicApiError::InvalidField("impl item name"))?;
            let path = join_path(owner_path, name)?;
            let kind = match first_key(inner(value)?)? {
                "function" => PublicApiKind::Method,
                "assoc_const" | "constant" => PublicApiKind::Constant,
                "assoc_type" | "type_alias" => PublicApiKind::TypeAlias,
                _ => continue,
            };
            self.insert_item(value, path.clone(), kind, path)?;
        }
        Ok(())
    }

    fn insert_item(
        &mut self,
        value: &Value,
        path: String,
        kind: PublicApiKind,
        owner_path: String,
    ) -> Result<(), PublicApiError> {
        self.insert_item_with_span(value, value, path, kind, owner_path)
    }

    fn insert_item_with_span(
        &mut self,
        value: &Value,
        fallback_span: &Value,
        path: String,
        kind: PublicApiKind,
        owner_path: String,
    ) -> Result<(), PublicApiError> {
        validate_api_path(&path)?;
        let (source_path, source_line) = source_span(value)
            .or_else(|| source_span(fallback_span))
            .ok_or_else(|| PublicApiError::MissingSpan { path: path.clone() })?;
        let key = (path.clone(), kind);
        let at_item_limit = self.items.len() >= MAX_PUBLIC_API_ITEMS;
        match self.items.entry(key) {
            std::collections::btree_map::Entry::Vacant(_) if at_item_limit => {
                Err(PublicApiError::TooManyItems {
                    actual: MAX_PUBLIC_API_ITEMS.saturating_add(1),
                    limit: MAX_PUBLIC_API_ITEMS,
                })
            }
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(ExtractedPublicApiItem {
                    crate_name: self.crate_name.to_string(),
                    path,
                    kind,
                    source_path,
                    source_line,
                    evidence_crate_name: self.crate_name.to_string(),
                    evidence_owner_path: owner_path.clone(),
                    owner_path,
                });
                Ok(())
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                let (path, kind) = entry.key();
                Err(PublicApiError::DuplicateIdentity {
                    path: path.clone(),
                    kind: *kind,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn rustdoc_extractor_tracks_reexports_methods_variants_and_trait_impls() {
        let value = json!({
            "format_version": 57,
            "includes_private": false,
            "root": 0,
            "index": {
                "0": item_json("stab_core", "public", "crates/stab-core/src/lib.rs", 1, json!({"module": {"items": [1, 2]}})),
                "1": item_json("inner", "public", "crates/stab-core/src/lib.rs", 2, json!({"module": {"items": [3]}})),
                "2": item_json(Option::<&str>::None, "public", "crates/stab-core/src/lib.rs", 3, json!({"use": {"source": "inner::Thing", "name": "Thing", "id": 3, "is_glob": false}})),
                "3": item_json("Thing", "public", "crates/stab-core/src/thing.rs", 4, json!({"enum": {"generics": {"params": [], "where_predicates": []}, "variants": [4], "impls": [5, 7], "has_stripped_variants": false}})),
                "4": item_json("A", "default", "crates/stab-core/src/thing.rs", 5, json!({"variant": {"kind": "plain", "discriminant": null}})),
                "5": item_json(Option::<&str>::None, "default", "crates/stab-core/src/thing.rs", 7, json!({"impl": {"is_unsafe": false, "generics": {"params": [], "where_predicates": []}, "provided_trait_methods": [], "trait": null, "for": {"resolved_path": {"path": "Thing", "id": 0, "args": null}}, "items": [6], "is_negative": false, "is_synthetic": false, "blanket_impl": null}})),
                "6": item_json("new", "public", "crates/stab-core/src/thing.rs", 8, json!({"function": {"sig": {"inputs": [], "output": null, "is_c_variadic": false}, "generics": {"params": [], "where_predicates": []}, "header": {"is_const": false, "is_unsafe": false, "is_async": false, "abi": "Rust"}, "has_body": true}})),
                "7": item_json(Option::<&str>::None, "default", "crates/stab-core/src/thing.rs", 9, json!({"impl": {"is_unsafe": false, "generics": {"params": [], "where_predicates": []}, "provided_trait_methods": [], "trait": {"resolved_path": {"path": "Clone", "id": 99, "args": null}}, "for": {"resolved_path": {"path": "Thing", "id": 0, "args": null}}, "items": [], "is_negative": false, "is_synthetic": false, "blanket_impl": null}}))
            }
        });

        let inventory = extract_rustdoc_inventory(&value, "stab_core").expect("valid rustdoc");
        let paths = inventory
            .items
            .iter()
            .map(|item| (item.path.as_str(), item.kind))
            .collect::<BTreeSet<_>>();
        assert!(paths.contains(&("stab_core::inner::Thing", PublicApiKind::Enum)));
        assert!(paths.contains(&("stab_core::inner::Thing::A", PublicApiKind::Variant)));
        assert!(paths.contains(&("stab_core::inner::Thing::new", PublicApiKind::Method)));
        assert!(paths.contains(&("stab_core::Thing", PublicApiKind::Enum)));
        assert!(paths.contains(&("stab_core::Thing::new", PublicApiKind::Method)));
        assert!(paths.iter().any(|(path, kind)| {
            path.starts_with("stab_core::Thing as Clone for@") && *kind == PublicApiKind::TraitImpl
        }));
    }

    #[test]
    fn rustdoc_extractor_resolves_external_stab_reexports() {
        let facade_json = json!({
            "format_version": 57,
            "includes_private": false,
            "root": 0,
            "index": {
                "0": item_json("stab_core", "public", "crates/stab-core/src/lib.rs", 1, json!({"module": {"items": [1]}})),
                "1": item_json(Option::<&str>::None, "public", "crates/stab-core/src/lib.rs", 3, json!({"use": {"source": "stab_bits::BitVec", "name": "BitVec", "id": 50, "is_glob": false}}))
            },
            "paths": {
                "50": {"crate_id": 1, "path": ["stab_bits", "BitVec"], "kind": "struct"}
            }
        });
        let mut facade =
            extract_rustdoc_inventory(&facade_json, "stab_core").expect("facade inventory");
        let dependency = RustdocInventory {
            crate_name: "stab_bits".to_string(),
            format_version: 57,
            items: vec![
                ExtractedPublicApiItem {
                    crate_name: "stab_bits".to_string(),
                    path: "stab_bits::BitVec".to_string(),
                    kind: PublicApiKind::Struct,
                    source_path: PathBuf::from("crates/stab-bits/src/lib.rs"),
                    source_line: 10,
                    owner_path: "stab_bits::BitVec".to_string(),
                    evidence_crate_name: "stab_bits".to_string(),
                    evidence_owner_path: "stab_bits::BitVec".to_string(),
                },
                ExtractedPublicApiItem {
                    crate_name: "stab_bits".to_string(),
                    path: "stab_bits::BitVec::zeros".to_string(),
                    kind: PublicApiKind::Method,
                    source_path: PathBuf::from("crates/stab-bits/src/lib.rs"),
                    source_line: 20,
                    owner_path: "stab_bits::BitVec::zeros".to_string(),
                    evidence_crate_name: "stab_bits".to_string(),
                    evidence_owner_path: "stab_bits::BitVec::zeros".to_string(),
                },
            ],
            visible_reexports: BTreeMap::new(),
            external_reexports: Vec::new(),
        };

        resolve_external_reexports(&mut facade, &[dependency]).expect("resolved facade");

        let paths = facade
            .items
            .iter()
            .map(|item| (item.path.as_str(), item.kind))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            paths,
            BTreeSet::from([
                ("stab_core::BitVec", PublicApiKind::Struct),
                ("stab_core::BitVec::zeros", PublicApiKind::Method),
            ])
        );
        assert!(facade.items.iter().all(|item| {
            item.evidence_crate_name == "stab_bits"
                && item
                    .evidence_owner_path
                    .replacen("stab_bits", "stab_core", 1)
                    == item.path
        }));
        assert!(facade.external_reexports.is_empty());
    }

    #[test]
    fn external_reexports_resolve_private_canonical_modules_through_visible_paths() {
        let facade_json = json!({
            "format_version": 57,
            "includes_private": false,
            "root": 0,
            "index": {
                "0": item_json("stab_core", "public", "crates/stab-core/src/lib.rs", 1, json!({"module": {"items": [1]}})),
                "1": item_json(Option::<&str>::None, "public", "crates/stab-core/src/lib.rs", 3, json!({"use": {"source": "stab_records::PackedShotBatch", "name": "PackedShotBatch", "id": 50, "is_glob": false}}))
            },
            "paths": {
                "50": {"crate_id": 1, "path": ["stab_records", "batches", "PackedShotBatch"], "kind": "struct"}
            }
        });
        let mut facade =
            extract_rustdoc_inventory(&facade_json, "stab_core").expect("facade inventory");
        let dependency = RustdocInventory {
            crate_name: "stab_records".to_string(),
            format_version: 57,
            items: vec![
                ExtractedPublicApiItem {
                    crate_name: "stab_records".to_string(),
                    path: "stab_records::PackedShotBatch".to_string(),
                    kind: PublicApiKind::Struct,
                    source_path: PathBuf::from("crates/stab-records/src/lib.rs"),
                    source_line: 10,
                    owner_path: "stab_records::PackedShotBatch".to_string(),
                    evidence_crate_name: "stab_records".to_string(),
                    evidence_owner_path: "stab_records::PackedShotBatch".to_string(),
                },
                ExtractedPublicApiItem {
                    crate_name: "stab_records".to_string(),
                    path: "stab_records::PackedShotBatch::zeros".to_string(),
                    kind: PublicApiKind::Method,
                    source_path: PathBuf::from("crates/stab-records/src/batches.rs"),
                    source_line: 12,
                    owner_path: "stab_records::PackedShotBatch::zeros".to_string(),
                    evidence_crate_name: "stab_records".to_string(),
                    evidence_owner_path: "stab_records::PackedShotBatch::zeros".to_string(),
                },
            ],
            visible_reexports: BTreeMap::from([(
                "stab_records::batches::PackedShotBatch".to_string(),
                "stab_records::PackedShotBatch".to_string(),
            )]),
            external_reexports: Vec::new(),
        };

        resolve_external_reexports(&mut facade, &[dependency]).expect("resolved facade");

        let paths = facade
            .items
            .iter()
            .map(|item| (item.path.as_str(), item.kind))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            paths,
            BTreeSet::from([
                ("stab_core::PackedShotBatch", PublicApiKind::Struct),
                ("stab_core::PackedShotBatch::zeros", PublicApiKind::Method),
            ])
        );
    }

    #[test]
    fn external_stab_reexport_fails_closed_without_canonical_inventory() {
        let mut facade = RustdocInventory {
            crate_name: "stab_core".to_string(),
            format_version: 57,
            items: Vec::new(),
            visible_reexports: BTreeMap::new(),
            external_reexports: vec![ExternalReexport {
                canonical_crate_name: "stab_records".to_string(),
                canonical_path: "stab_records::SampleFormat".to_string(),
                alias_path: "stab_core::SampleFormat".to_string(),
                source_path: PathBuf::from("crates/stab-core/src/lib.rs"),
                source_line: 2,
            }],
        };

        let error = resolve_external_reexports(&mut facade, &[])
            .expect_err("missing canonical inventory must fail");
        assert!(matches!(
            error,
            PublicApiError::UnresolvedExternalReexport { .. }
        ));
    }

    #[test]
    fn trait_impl_identity_includes_generic_arguments() {
        let from_u8 = json!({
            "path": "From",
            "id": 1,
            "args": {"angle_bracketed": {"args": [{"type": {"primitive": "u8"}}], "constraints": []}}
        });
        let from_u16 = json!({
            "path": "From",
            "id": 1,
            "args": {"angle_bracketed": {"args": [{"type": {"primitive": "u16"}}], "constraints": []}}
        });
        let first = resolved_path_name(&from_u8, &Map::new()).expect("valid generic trait");
        let second = resolved_path_name(&from_u16, &Map::new()).expect("valid generic trait");
        assert!(first.starts_with("From@"));
        assert_ne!(first, second);
    }

    #[test]
    fn trait_impl_identity_qualifies_same_named_types_from_different_crates() {
        let local = json!({
            "path": "From",
            "id": 1,
            "args": {"angle_bracketed": {"args": [{"type": {"resolved_path": {"path": "FormatError", "id": 2, "args": null}}}], "constraints": []}}
        });
        let external = json!({
            "path": "From",
            "id": 1,
            "args": {"angle_bracketed": {"args": [{"type": {"resolved_path": {"path": "FormatError", "id": 3, "args": null}}}], "constraints": []}}
        });
        let paths = Map::from_iter([
            (
                "2".to_string(),
                json!({"crate_id": 0, "path": ["stab_core", "FormatError"], "kind": "struct"}),
            ),
            (
                "3".to_string(),
                json!({"crate_id": 1, "path": ["stab_records", "FormatError"], "kind": "struct"}),
            ),
        ]);

        let first = resolved_path_name(&local, &paths).expect("valid local trait");
        let second = resolved_path_name(&external, &paths).expect("valid external trait");

        assert_ne!(first, second);
    }

    #[test]
    fn rustdoc_extractor_excludes_hidden_synthetic_and_blanket_items() {
        let mut hidden = item_json(
            "evidence_only",
            "public",
            "crates/stab-core/src/lib.rs",
            3,
            json!({"function": {"sig": {"inputs": [], "output": null, "is_c_variadic": false}, "generics": {"params": [], "where_predicates": []}, "header": {"is_const": false, "is_unsafe": false, "is_async": false, "abi": "Rust"}, "has_body": true}}),
        );
        *hidden.get_mut("attrs").expect("item attrs") = json!([{"other": "#[doc(hidden)]"}]);
        let value = json!({
            "format_version": 57,
            "includes_private": false,
            "root": 0,
            "index": {
                "0": item_json("stab_core", "public", "crates/stab-core/src/lib.rs", 1, json!({"module": {"items": [1, 2]}})),
                "1": item_json("Thing", "public", "crates/stab-core/src/thing.rs", 2, json!({"struct": {"kind": {"unit": null}, "generics": {"params": [], "where_predicates": []}, "impls": [3, 4]} })),
                "2": hidden,
                "3": item_json(Option::<&str>::None, "default", "crates/stab-core/src/thing.rs", 4, json!({"impl": {"is_unsafe": false, "generics": {"params": [], "where_predicates": []}, "provided_trait_methods": [], "trait": {"resolved_path": {"path": "Send", "id": 99, "args": null}}, "for": {"resolved_path": {"path": "Thing", "id": 1, "args": null}}, "items": [], "is_negative": false, "is_synthetic": true, "blanket_impl": null}})),
                "4": item_json(Option::<&str>::None, "default", "crates/stab-core/src/thing.rs", 5, json!({"impl": {"is_unsafe": false, "generics": {"params": [], "where_predicates": []}, "provided_trait_methods": [], "trait": {"resolved_path": {"path": "Into", "id": 98, "args": null}}, "for": {"resolved_path": {"path": "Thing", "id": 1, "args": null}}, "items": [], "is_negative": false, "is_synthetic": false, "blanket_impl": {"generic": "T"}}}))
            }
        });

        let inventory = extract_rustdoc_inventory(&value, "stab_core").expect("valid rustdoc");
        let paths = inventory
            .items
            .iter()
            .map(|item| item.path.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(paths, BTreeSet::from(["stab_core::Thing"]));
    }

    #[test]
    fn rustdoc_extractor_rejects_default_feature_evidence_exports() {
        let value = json!({
            "format_version": 57,
            "includes_private": false,
            "root": 0,
            "index": {
                "0": item_json("stab_core", "public", "crates/stab-core/src/lib.rs", 1, json!({"module": {"items": [1]}})),
                "1": item_json("__gate_contract_family_names", "public", "crates/stab-core/src/gate.rs", 2, json!({"function": {"sig": {"inputs": [], "output": null, "is_c_variadic": false}, "generics": {"params": [], "where_predicates": []}, "header": {"is_const": false, "is_unsafe": false, "is_async": false, "abi": "Rust"}, "has_body": true}}))
            }
        });
        let error = extract_rustdoc_inventory(&value, "stab_core")
            .expect_err("evidence-only default export must fail");
        assert!(matches!(error, PublicApiError::EvidenceOnlyExport(_)));
    }

    #[test]
    fn rustdoc_extractor_tracks_named_and_tuple_enum_payload_fields() {
        let value = json!({
            "format_version": 57,
            "includes_private": false,
            "root": 0,
            "index": {
                "0": item_json("stab_core", "public", "crates/stab-core/src/lib.rs", 1, json!({"module": {"items": [1]}})),
                "1": item_json("Failure", "public", "crates/stab-core/src/error.rs", 2, json!({"enum": {"generics": {"params": [], "where_predicates": []}, "variants": [2, 4], "impls": [], "has_stripped_variants": false}})),
                "2": item_json("Named", "default", "crates/stab-core/src/error.rs", 3, json!({"variant": {"kind": {"struct": {"fields": [3], "has_stripped_fields": false}}, "discriminant": null}})),
                "3": item_json("message", "default", "crates/stab-core/src/error.rs", 3, json!({"struct_field": {"primitive": "str"}})),
                "4": item_json("Tuple", "default", "crates/stab-core/src/error.rs", 4, json!({"variant": {"kind": {"tuple": [5]}, "discriminant": null}})),
                "5": item_json("0", "default", "crates/stab-core/src/error.rs", 4, json!({"struct_field": {"primitive": "u64"}}))
            }
        });

        let inventory = extract_rustdoc_inventory(&value, "stab_core").expect("valid rustdoc");
        let paths = inventory
            .items
            .iter()
            .map(|item| (item.path.as_str(), item.kind, item.owner_path.as_str()))
            .collect::<BTreeSet<_>>();
        assert!(paths.contains(&(
            "stab_core::Failure::Named::message",
            PublicApiKind::Field,
            "stab_core::Failure"
        )));
        assert!(paths.contains(&(
            "stab_core::Failure::Tuple::0",
            PublicApiKind::Field,
            "stab_core::Failure"
        )));
    }

    #[test]
    fn trait_impl_identity_includes_self_specialization_without_rustdoc_ids() {
        let first = json!({"resolved_path": {"path": "Thing", "id": 10, "args": {"angle_bracketed": {"args": [{"type": {"resolved_path": {"path": "Alpha", "id": 100, "args": null}}}], "constraints": []}}}});
        let same_semantics_new_ids = json!({"resolved_path": {"path": "Thing", "id": 11, "args": {"angle_bracketed": {"args": [{"type": {"resolved_path": {"path": "Alpha", "id": 101, "args": null}}}], "constraints": []}}}});
        let second = json!({"resolved_path": {"path": "Thing", "id": 10, "args": {"angle_bracketed": {"args": [{"type": {"resolved_path": {"path": "Beta", "id": 100, "args": null}}}], "constraints": []}}}});
        assert_eq!(
            canonical_value_digest(&first),
            canonical_value_digest(&same_semantics_new_ids)
        );
        assert_ne!(
            canonical_value_digest(&first),
            canonical_value_digest(&second)
        );
    }

    #[test]
    fn rustdoc_extractor_emits_trait_impl_only_for_implementing_type() {
        let value = json!({
            "format_version": 57,
            "includes_private": false,
            "root": 0,
            "index": {
                "0": with_id(item_json("stab_core", "public", "crates/stab-core/src/lib.rs", 1, json!({"module": {"items": [1, 3]}})), 0),
                "1": with_id(item_json("Argument", "public", "crates/stab-core/src/argument.rs", 2, json!({"struct": {"kind": {"unit": null}, "generics": {"params": [], "where_predicates": []}, "impls": [2]}})), 1),
                "2": with_id(item_json(Option::<&str>::None, "default", "crates/stab-core/src/result.rs", 3, json!({"impl": {"is_unsafe": false, "generics": {"params": [], "where_predicates": []}, "provided_trait_methods": [], "trait": {"resolved_path": {"path": "From", "id": 99, "args": {"angle_bracketed": {"args": [{"type": {"resolved_path": {"path": "Argument", "id": 1, "args": null}}}], "constraints": []}}}}, "for": {"resolved_path": {"path": "Result", "id": 3, "args": null}}, "items": [], "is_negative": false, "is_synthetic": false, "blanket_impl": null}})), 2),
                "3": with_id(item_json("Result", "public", "crates/stab-core/src/result.rs", 4, json!({"struct": {"kind": {"unit": null}, "generics": {"params": [], "where_predicates": []}, "impls": [2]}})), 3)
            }
        });

        let inventory = extract_rustdoc_inventory(&value, "stab_core").expect("valid rustdoc");
        let impl_paths = inventory
            .items
            .iter()
            .filter(|item| item.kind == PublicApiKind::TraitImpl)
            .map(|item| item.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(impl_paths.len(), 1);
        assert!(
            impl_paths
                .first()
                .is_some_and(|path| path.starts_with("stab_core::Result as From@"))
        );
    }

    #[test]
    fn rustdoc_extractor_bounds_cyclic_glob_reexports() {
        let value = json!({
            "format_version": 57,
            "includes_private": false,
            "root": 0,
            "index": {
                "0": item_json("stab_core", "public", "crates/stab-core/src/lib.rs", 1, json!({"module": {"items": [1]}})),
                "1": item_json("a", "public", "crates/stab-core/src/lib.rs", 2, json!({"module": {"items": [2]}})),
                "2": item_json(Option::<&str>::None, "public", "crates/stab-core/src/lib.rs", 3, json!({"use": {"source": "crate::b::*", "name": "b", "id": 3, "is_glob": true}})),
                "3": item_json("b", "public", "crates/stab-core/src/lib.rs", 4, json!({"module": {"items": [4, 5]}})),
                "4": item_json(Option::<&str>::None, "public", "crates/stab-core/src/lib.rs", 5, json!({"use": {"source": "crate::a::*", "name": "a", "id": 1, "is_glob": true}})),
                "5": item_json("value", "public", "crates/stab-core/src/lib.rs", 6, json!({"function": {"sig": {"inputs": [], "output": null, "is_c_variadic": false}, "generics": {"params": [], "where_predicates": []}, "header": {"is_const": false, "is_unsafe": false, "is_async": false, "abi": "Rust"}, "has_body": true}}))
            }
        });

        let inventory = extract_rustdoc_inventory(&value, "stab_core").expect("bounded cycle");
        assert_eq!(inventory.items.len(), 1);
        assert_eq!(
            inventory.items.first().expect("glob-exported value").path,
            "stab_core::a::value"
        );
    }

    fn item_json(
        name: impl serde::Serialize,
        visibility: &str,
        filename: &str,
        line: u32,
        inner: Value,
    ) -> Value {
        json!({
            "id": 0,
            "crate_id": 0,
            "name": name,
            "span": {"filename": filename, "begin": [line, 1], "end": [line, 2]},
            "visibility": visibility,
            "docs": null,
            "links": {},
            "attrs": [],
            "deprecation": null,
            "inner": inner
        })
    }

    fn with_id(mut value: Value, id: u64) -> Value {
        value
            .as_object_mut()
            .expect("rustdoc test item")
            .insert("id".to_string(), json!(id));
        value
    }
}
