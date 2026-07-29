use super::InventoryError;
use crate::RepoRoot;
use crate::qualification::public_api::{
    ExtractedPublicApiItem, PublicApiError, ResolvedExternalReexport, generate_rustdoc_inventory,
    resolve_external_reexports,
};

pub(super) struct ExtractedApis {
    pub(super) items: Vec<ExtractedPublicApiItem>,
    pub(super) external_aliases: Vec<ResolvedExternalReexport>,
}

pub(super) fn extract(root: &RepoRoot) -> Result<ExtractedApis, InventoryError> {
    let kernels = generate_rustdoc_inventory(&root.path, "stab-kernels-simd", "stab_kernels_simd")?;
    let bits = generate_rustdoc_inventory(&root.path, "stab-bits", "stab_bits")?;
    let records = generate_rustdoc_inventory(&root.path, "stab-records", "stab_records")?;
    let mut algebra = generate_rustdoc_inventory(&root.path, "stab-algebra", "stab_algebra")?;
    let mut external_aliases =
        resolve_external_reexports(&mut algebra, std::slice::from_ref(&bits))?;
    let mut model = generate_rustdoc_inventory(&root.path, "stab-model", "stab_model")?;
    external_aliases.extend(resolve_external_reexports(
        &mut model,
        std::slice::from_ref(&algebra),
    )?);
    let mut analysis = generate_rustdoc_inventory(&root.path, "stab-analysis", "stab_analysis")?;
    external_aliases.extend(resolve_external_reexports(
        &mut analysis,
        &[algebra.clone(), model.clone()],
    )?);
    let engine = generate_rustdoc_inventory(&root.path, "stab-engine", "stab_engine")?;
    let mut facade = generate_rustdoc_inventory(&root.path, "stab-core", "stab_core")?;
    external_aliases.extend(resolve_external_reexports(
        &mut facade,
        &[
            bits.clone(),
            records.clone(),
            algebra.clone(),
            model.clone(),
            analysis.clone(),
            engine.clone(),
        ],
    )?);
    if facade.format_version != kernels.format_version
        || facade.format_version != bits.format_version
        || facade.format_version != records.format_version
        || facade.format_version != algebra.format_version
        || facade.format_version != model.format_version
        || facade.format_version != analysis.format_version
        || facade.format_version != engine.format_version
    {
        return Err(PublicApiError::InvalidField("rustdoc format version mismatch").into());
    }

    facade.items.extend(kernels.items);
    facade.items.extend(bits.items);
    facade.items.extend(records.items);
    facade.items.extend(algebra.items);
    facade.items.extend(model.items);
    facade.items.extend(analysis.items);
    facade.items.extend(engine.items);
    let cli = generate_rustdoc_inventory(&root.path, "stab-cli", "stab_cli")?;
    if facade.format_version != cli.format_version {
        return Err(PublicApiError::InvalidField("rustdoc format version mismatch").into());
    }
    facade.items.extend(cli.items);
    facade.items.sort();
    Ok(ExtractedApis {
        items: facade.items,
        external_aliases,
    })
}
