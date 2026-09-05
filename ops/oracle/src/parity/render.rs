use std::collections::BTreeMap;
use std::path::Path;

use super::{
    Area, DetsObservableOrder, Disposition, Evidence, EvidenceStatus, Family, GENERATED_DOC_PATH,
    Ledger, MAX_GENERATED_DOC_BYTES, ParityError, Status, read_regular_file_bounded,
};
use crate::RepoRoot;
use crate::safe_file;

pub(super) fn render_document(
    root: &RepoRoot,
    ledger: &Ledger,
    check: bool,
) -> Result<(), ParityError> {
    let rendered = render(ledger);
    let path = root.path.join(GENERATED_DOC_PATH);
    if check {
        let current = read_regular_file_bounded(&path, MAX_GENERATED_DOC_BYTES)?;
        if current != rendered.as_bytes() {
            return Err(ParityError::GeneratedDocumentDiffers(
                path.into_boxed_path(),
            ));
        }
        println!("[stab-oracle] generated Stim parity document is clean");
        return Ok(());
    }
    atomic_write(&path, rendered.as_bytes())?;
    println!("[stab-oracle] wrote {GENERATED_DOC_PATH}");
    Ok(())
}

pub(super) fn render(ledger: &Ledger) -> String {
    let mut counts = BTreeMap::<Status, usize>::new();
    let mut evidence_counts = BTreeMap::<EvidenceStatus, usize>::new();
    for family in &ledger.families {
        *counts.entry(family.status()).or_default() += 1;
        *evidence_counts.entry(family.evidence_status()).or_default() += 1;
    }
    let mut output = String::new();
    output.push_str("# Stim v1.16.0 Core Parity\n\n");
    output.push_str(
        "This file is generated from `oracle/stim-v1.16-parity.toml`. Do not edit it directly.\n\n",
    );
    output.push_str(&format!(
        "Pinned target: `{}` at `{}`.\n\n",
        ledger.stim.version, ledger.stim.commit
    ));
    output.push_str("The ledger's `required_fixture_ids` preserves named supporting fixtures from [the fixture corpus](../oracle/fixtures/manifest.csv). Each reference must resolve to one implemented fixture; these requirements do not replace canonical family evidence.\n\n");
    output.push_str("## Summary\n\n");
    output.push_str("| Status | Families |\n| --- | ---: |\n");
    for status in [
        Status::Done,
        Status::Missing,
        Status::Deferred,
        Status::Divergence,
    ] {
        output.push_str(&format!(
            "| {} | {} |\n",
            status.as_str(),
            counts.get(&status).copied().unwrap_or_default()
        ));
    }
    output.push_str("\nStatus describes implementation only. A `done` row may still need a lean canonical owner before it is qualification-ready; `missing` means the behavior contract itself is incomplete. A `deferred` or `divergence` row states why it is outside exact parity.\n\n");
    output.push_str("| Evidence | Families |\n| --- | ---: |\n");
    for status in [
        EvidenceStatus::Verified,
        EvidenceStatus::NeedsOwner,
        EvidenceStatus::NotApplicable,
    ] {
        output.push_str(&format!(
            "| {} | {} |\n",
            status.as_str(),
            evidence_counts.get(&status).copied().unwrap_or_default()
        ));
    }

    output.push_str("\n## Result Format Applicability\n\n");
    output.push_str("| Route | Command | Role | Record types | DETS observable order | Accepted | Rejected like Stim | Stab fixes Stim bug | Stim references |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for route in &ledger.format_routes {
        let record_types = route
            .record_types
            .iter()
            .map(|record_type| format!("`{}`", record_type.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        let accepted = format_list(&route.accepted_formats);
        let rejected = format_list(&route.rejected_formats);
        let divergences = format_list(&route.stim_bug_divergences);
        let dets_observable_order = route
            .dets_observable_order
            .map_or("-", DetsObservableOrder::as_str);
        let refs = route
            .stim_refs
            .iter()
            .map(|reference| format!("[`{reference}`](../vendor/stim/{reference})"))
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "| `{}` | `{}` | {} | {} | {} | {} | {} | {} | {} |\n",
            route.id,
            route.command,
            route.role.as_str(),
            record_types,
            dets_observable_order,
            accepted,
            rejected,
            divergences,
            refs
        ));
    }

    output.push_str("\n## Computational Command Options\n\n");
    output.push_str("| Command | Nondeprecated options | Stim references |\n| --- | --- | --- |\n");
    for surface in &ledger.command_surfaces {
        let options = surface
            .options
            .iter()
            .map(|option| format!("`{option}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let refs = surface
            .stim_refs
            .iter()
            .map(|reference| format!("[`{reference}`](../vendor/stim/{reference})"))
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "| `{}` | {} | {} |\n",
            surface.command, options, refs
        ));
    }

    let mut areas = BTreeMap::<Area, Vec<&Family>>::new();
    for family in &ledger.families {
        areas.entry(family.area).or_default().push(family);
    }
    for (area, families) in areas {
        output.push_str(&format!(
            "\n## {}\n\n| Family | Status | Evidence | Owner | Contract | Disposition | Stim references |\n| --- | --- | --- | --- | --- | --- | --- |\n",
            title(area.as_str())
        ));
        for family in families {
            let disposition = match &family.disposition {
                Disposition::Done {
                    evidence: Evidence::Verified { test, .. },
                    ..
                } => format!("`{}`", test.display()),
                Disposition::Done {
                    evidence: Evidence::NeedsOwner { milestone },
                    ..
                } => {
                    format!("Canonical owner due in {}", milestone.as_str())
                }
                Disposition::Missing { milestone, .. } => {
                    format!("Finish in {}", milestone.as_str())
                }
                Disposition::Deferred { rationale } => rationale.clone(),
                Disposition::Divergence {
                    divergence_kind,
                    rationale,
                    evidence:
                        Evidence::Verified {
                            test,
                            stim_reproduction,
                        },
                    ..
                } => {
                    let reproduction = stim_reproduction
                        .as_ref()
                        .map_or_else(String::new, |owner| {
                            format!(" Pinned reproduction: `{}`.", owner.display())
                        });
                    format!(
                        "{}: {} `{}`.{reproduction}",
                        divergence_kind.as_str(),
                        rationale,
                        test.display()
                    )
                }
                Disposition::Divergence {
                    divergence_kind,
                    rationale,
                    evidence: Evidence::NeedsOwner { milestone },
                    ..
                } => format!(
                    "{}: {} Canonical owner due in {}",
                    divergence_kind.as_str(),
                    rationale,
                    milestone.as_str()
                ),
            };
            let refs = family
                .stim_refs
                .iter()
                .map(|reference| format!("[`{reference}`](../vendor/stim/{reference})"))
                .collect::<Vec<_>>()
                .join(", ");
            output.push_str(&format!(
                "| `{}` | {} | {} | {} | {} | {} | {} |\n",
                family.id,
                family.status().as_str(),
                family.evidence_status().as_str(),
                family
                    .owner()
                    .map_or_else(|| "-".to_string(), |owner| format!("`{owner}`")),
                escape_table(&family.contract),
                escape_table(&disposition),
                refs
            ));
        }
    }
    output
}

fn format_list(formats: &[String]) -> String {
    if formats.is_empty() {
        return "-".to_string();
    }
    formats
        .iter()
        .map(|format| format!("`{format}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn escape_table(value: &str) -> String {
    value.replace('|', "\\|")
}

fn title(slug: &str) -> String {
    slug.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), ParityError> {
    safe_file::atomic_write_regular_file(path, bytes).map_err(|source| ParityError::SafeFile {
        action: "atomically write",
        path: path.to_path_buf().into_boxed_path(),
        source,
    })
}
