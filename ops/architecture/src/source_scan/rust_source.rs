use std::collections::BTreeSet;

use proc_macro2::{TokenStream, TokenTree};
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{Item, Meta, Token, UseTree};

#[derive(Default)]
pub(super) struct SourceFacts {
    pub(super) contains_portable_simd: bool,
    pub(super) feature_gates: BTreeSet<String>,
    pub(super) has_macro_export: bool,
}

pub(super) fn inspect(source: &str) -> syn::Result<SourceFacts> {
    let syntax = syn::parse_file(source)?;
    let mut inspector = SourceInspector {
        current_aliases: BTreeSet::new(),
        parent_aliases: Vec::new(),
        facts: SourceFacts::default(),
        attribute_error: None,
    };
    inspector.visit_file(&syntax);
    if let Some(error) = inspector.attribute_error {
        return Err(error);
    }
    Ok(inspector.facts)
}

fn standard_root_aliases<'a>(items: impl IntoIterator<Item = &'a Item>) -> BTreeSet<String> {
    let items = items.into_iter().collect::<Vec<_>>();
    aliases_for_items(
        &items,
        ["core".to_owned(), "std".to_owned()].into_iter().collect(),
    )
}

fn aliases_for_items(items: &[&Item], mut aliases: BTreeSet<String>) -> BTreeSet<String> {
    loop {
        let mut discovered = BTreeSet::new();
        for item in items {
            match **item {
                Item::Use(ref item) => collect_standard_aliases_from_use_tree(
                    &item.tree,
                    &mut Vec::new(),
                    &aliases,
                    &mut discovered,
                ),
                Item::ExternCrate(ref item)
                    if matches!(item.ident.to_string().as_str(), "core" | "std") =>
                {
                    let alias = item
                        .rename
                        .as_ref()
                        .map_or_else(|| item.ident.to_string(), |(_, rename)| rename.to_string());
                    if alias != "_" {
                        discovered.insert(alias);
                    }
                }
                _ => {}
            }
        }
        let previous_len = aliases.len();
        aliases.extend(discovered);
        if aliases.len() == previous_len {
            return aliases;
        }
    }
}

fn collect_standard_aliases_from_use_tree(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    known_aliases: &BTreeSet<String>,
    discovered_aliases: &mut BTreeSet<String>,
) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_standard_aliases_from_use_tree(
                &path.tree,
                prefix,
                known_aliases,
                discovered_aliases,
            );
            prefix.pop();
        }
        UseTree::Rename(rename) => {
            let mut source = prefix.clone();
            if rename.ident != "self" {
                source.push(rename.ident.to_string());
            }
            if source.len() == 1
                && source
                    .first()
                    .is_some_and(|root| known_aliases.contains(root))
                && rename.rename != "_"
            {
                discovered_aliases.insert(rename.rename.to_string());
            }
        }
        UseTree::Group(group) => {
            for tree in &group.items {
                collect_standard_aliases_from_use_tree(
                    tree,
                    prefix,
                    known_aliases,
                    discovered_aliases,
                );
            }
        }
        UseTree::Name(_) | UseTree::Glob(_) => {}
    }
}

struct SourceInspector {
    current_aliases: BTreeSet<String>,
    parent_aliases: Vec<BTreeSet<String>>,
    facts: SourceFacts,
    attribute_error: Option<syn::Error>,
}

impl SourceInspector {
    fn active_aliases(&self) -> &BTreeSet<String> {
        &self.current_aliases
    }

    fn enter_scope(&mut self, aliases: BTreeSet<String>) {
        let previous = std::mem::replace(&mut self.current_aliases, aliases);
        self.parent_aliases.push(previous);
    }

    fn leave_scope(&mut self) {
        let Some(previous) = self.parent_aliases.pop() else {
            self.attribute_error = Some(syn::Error::new(
                proc_macro2::Span::call_site(),
                "source inspector lost its lexical alias parent",
            ));
            return;
        };
        self.current_aliases = previous;
    }

    fn inspect_attribute(&mut self, attribute: &syn::Attribute) {
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
            return;
        }
        self.facts.contains_portable_simd |= self.facts.feature_gates.contains("portable_simd");
    }
}

impl<'ast> Visit<'ast> for SourceInspector {
    fn visit_file(&mut self, file: &'ast syn::File) {
        self.current_aliases = standard_root_aliases(file.items.iter());
        for attribute in &file.attrs {
            self.visit_attribute(attribute);
        }
        for item in &file.items {
            self.visit_item(item);
        }
    }

    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        self.inspect_attribute(attribute);
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        let items = block
            .stmts
            .iter()
            .filter_map(|statement| match statement {
                syn::Stmt::Item(item) => Some(item),
                _ => None,
            })
            .collect::<Vec<_>>();
        let aliases = aliases_for_items(&items, self.active_aliases().clone());
        self.enter_scope(aliases);
        visit::visit_block(self, block);
        self.leave_scope();
    }

    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        for attribute in &item.attrs {
            self.visit_attribute(attribute);
        }
        if let Some((_, items)) = &item.content {
            self.enter_scope(standard_root_aliases(items.iter()));
            for item in items {
                self.visit_item(item);
            }
            self.leave_scope();
        }
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.facts.contains_portable_simd |=
            use_tree_contains_portable_simd(&item.tree, &mut Vec::new(), self.active_aliases());
        visit::visit_item_use(self, item);
    }

    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        self.facts.contains_portable_simd |=
            token_stream_contains_portable_simd(&item.tokens, self.active_aliases());
        visit::visit_macro(self, item);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        self.facts.contains_portable_simd |= path_segments_contain_portable_simd(
            path.segments
                .iter()
                .map(|segment| segment.ident.to_string()),
            self.active_aliases(),
        );
        visit::visit_path(self, path);
    }
}

fn token_stream_contains_portable_simd(
    tokens: &TokenStream,
    root_aliases: &BTreeSet<String>,
) -> bool {
    let tokens = tokens.clone().into_iter().collect::<Vec<_>>();
    if tokens.windows(4).any(|window| {
        matches!(
            window,
            [
                TokenTree::Ident(root),
                TokenTree::Punct(first_colon),
                TokenTree::Punct(second_colon),
                TokenTree::Ident(module),
            ] if root_aliases.contains(&root.to_string())
                && first_colon.as_char() == ':'
                && second_colon.as_char() == ':'
                && module == "simd"
        )
    }) {
        return true;
    }
    tokens.into_iter().any(|token| {
        matches!(
            token,
            TokenTree::Group(group)
                if token_stream_contains_portable_simd(&group.stream(), root_aliases)
        )
    })
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

fn use_tree_contains_portable_simd(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    root_aliases: &BTreeSet<String>,
) -> bool {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            let found = path_segments_contain_portable_simd(
                prefix.iter().map(String::as_str),
                root_aliases,
            ) || use_tree_contains_portable_simd(&path.tree, prefix, root_aliases);
            prefix.pop();
            found
        }
        UseTree::Name(name) => {
            let include_name = name.ident != "self";
            if include_name {
                prefix.push(name.ident.to_string());
            }
            let found = path_segments_contain_portable_simd(
                prefix.iter().map(String::as_str),
                root_aliases,
            );
            if include_name {
                prefix.pop();
            }
            found
        }
        UseTree::Rename(rename) => {
            let include_name = rename.ident != "self";
            if include_name {
                prefix.push(rename.ident.to_string());
            }
            let found = path_segments_contain_portable_simd(
                prefix.iter().map(String::as_str),
                root_aliases,
            );
            if include_name {
                prefix.pop();
            }
            found
        }
        UseTree::Glob(_) => {
            path_segments_contain_portable_simd(prefix.iter().map(String::as_str), root_aliases)
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .any(|tree| use_tree_contains_portable_simd(tree, prefix, root_aliases)),
    }
}

fn path_segments_contain_portable_simd<I, S>(segments: I, root_aliases: &BTreeSet<String>) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut segments = segments.into_iter();
    let Some(root) = segments.next() else {
        return false;
    };
    let Some(module) = segments.next() else {
        return false;
    };
    root_aliases.contains(root.as_ref()) && module.as_ref() == "simd"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_portable_simd(source: &str) -> bool {
        inspect(source)
            .expect("fixture must parse")
            .contains_portable_simd
    }

    #[test]
    fn finds_direct_grouped_feature_gated_and_macro_portable_simd() {
        for source in [
            "use std::simd::Simd;",
            "use std::{mem, simd::{Simd}};",
            "use core::simd::Simd;",
            "use core::{mem, simd::{Simd}};",
            "#![feature(portable_simd)]",
            "#![cfg_attr(all(), cfg_attr(any(), feature(portable_simd)))]",
            "macro_rules! kernel { () => {{ let _ = std::simd::Simd::<u64, 4>::splat(0); }} }",
            "consume_tokens!({ core::simd::Simd::<u64, 4>::splat(0) });",
        ] {
            assert!(has_portable_simd(source), "{source}");
        }
    }

    #[test]
    fn finds_lexically_scoped_standard_aliases() {
        for source in [
            "use std as platform;\nuse platform::simd::Simd;",
            "use std::{self as platform, mem};\ntype Lanes = platform::simd::Simd<u64, 4>;",
            "use core as platform;\nuse platform as foundation;\nuse foundation::simd::*;",
            "extern crate core as platform;\ntype Mask = platform::simd::Mask<i64, 4>;",
            "fn kernel() { use std as platform; let _ = platform::simd::Simd::<u64, 4>::splat(0); }",
        ] {
            assert!(has_portable_simd(source), "{source}");
        }
    }

    #[test]
    fn does_not_leak_aliases_between_modules() {
        let source = r#"
            mod standard_owner {
                use std as platform;
            }
            mod unrelated_owner {
                use crate::platform;
                type Local = platform::simd::Local;
            }
        "#;
        assert!(!has_portable_simd(source));
    }

    #[test]
    fn ignores_comments_strings_similar_names_and_unrelated_aliases() {
        let unrelated = r#"
            use crate::std as platform;
            use crate::simd;
            const TEXT: &str = "std::simd and #![feature(portable_simd)]";
            const MACRO_TEXT: &str = "macro_rules! x { () => { std::simd::Simd } }";
            // use core::simd::Simd;
            fn f() {
                let std_simd = 1;
                let _ = platform::simd::Local;
            }
        "#;
        assert!(!has_portable_simd(unrelated));
        assert!(!has_portable_simd("use std as platform;"));
    }

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
        assert!(!facts.contains_portable_simd);
    }

    #[test]
    fn malformed_rust_fails_closed() {
        assert!(inspect("fn broken( {").is_err());
    }
}
