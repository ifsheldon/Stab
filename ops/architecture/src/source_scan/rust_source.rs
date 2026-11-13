use std::collections::BTreeSet;

use syn::punctuated::Punctuated;
use syn::visit::Visit;
use syn::{Meta, Token};

#[derive(Default)]
pub(super) struct SourceFacts {
    pub(super) feature_gates: BTreeSet<String>,
    pub(super) has_macro_export: bool,
}

pub(super) fn inspect(source: &str) -> syn::Result<SourceFacts> {
    let syntax = syn::parse_file(source)?;
    let mut inspector = SourceInspector {
        facts: SourceFacts::default(),
        attribute_error: None,
    };
    inspector.visit_file(&syntax);
    if let Some(error) = inspector.attribute_error {
        return Err(error);
    }
    Ok(inspector.facts)
}

struct SourceInspector {
    facts: SourceFacts,
    attribute_error: Option<syn::Error>,
}

impl<'ast> Visit<'ast> for SourceInspector {
    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        if self.attribute_error.is_some() {
            return;
        }
        match meta_has_macro_export(&attribute.meta) {
            Ok(has_macro_export) => self.facts.has_macro_export |= has_macro_export,
            Err(error) => {
                self.attribute_error = Some(error);
                return;
            }
        }
        if let Err(error) = collect_feature_gates(&attribute.meta, &mut self.facts.feature_gates) {
            self.attribute_error = Some(error);
        }
    }
}

fn meta_has_macro_export(meta: &Meta) -> syn::Result<bool> {
    if matches!(meta, Meta::Path(path) if path.is_ident("macro_export")) {
        return Ok(true);
    }
    let Meta::List(list) = meta else {
        return Ok(false);
    };
    if !list.path.is_ident("cfg_attr") {
        return Ok(false);
    }
    let nested = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
    for meta in nested {
        if meta_has_macro_export(&meta)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn collect_feature_gates(meta: &Meta, output: &mut BTreeSet<String>) -> syn::Result<()> {
    let Meta::List(list) = meta else {
        return Ok(());
    };
    if list.path.is_ident("feature") {
        let features =
            list.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated)?;
        output.extend(features.iter().map(|feature| {
            feature
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::")
        }));
        return Ok(());
    }
    if !list.path.is_ident("cfg_attr") {
        return Ok(());
    }

    let nested = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
    for meta in nested {
        collect_feature_gates(&meta, output)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_all_feature_gates_and_macro_exports() {
        let facts = inspect(
            "#![cfg_attr(target_os = \"none\", feature(allocator_api))]\n#[cfg_attr(target_os = \"none\", macro_export)]\nmacro_rules! exported { () => {} }",
        )
        .expect("fixture should parse");

        assert_eq!(
            facts.feature_gates,
            BTreeSet::from(["allocator_api".to_owned()])
        );
        assert!(facts.has_macro_export);
    }

    #[test]
    fn records_nested_and_inner_module_feature_gates() {
        let facts = inspect(
            "#![cfg_attr(all(), cfg_attr(any(), feature(generic_const_exprs)))]\nmod inner { #![feature(allocator_api)] }\nfn f() { #[cfg_attr(test, feature(never_type))] fn g() {} }",
        )
        .expect("fixture should parse");

        assert_eq!(
            facts.feature_gates,
            BTreeSet::from([
                "allocator_api".to_owned(),
                "never_type".to_owned(),
                "generic_const_exprs".to_owned(),
            ])
        );
    }

    #[test]
    fn ignores_feature_gates_in_comments_and_strings() {
        let facts = inspect(
            "// #![feature(generic_const_exprs)]\nconst TEXT: &str = \"#![feature(generic_const_exprs)]\";",
        )
        .expect("fixture should parse");

        assert!(facts.feature_gates.is_empty());
        assert!(!facts.has_macro_export);
    }

    #[test]
    fn malformed_rust_fails_closed() {
        assert!(inspect("fn broken( {").is_err());
    }
}
