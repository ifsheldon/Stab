# Stab

## Development

Install the local staged-aware Git hook with:

```sh
just maintenance::setup-hooks
```

Run the same checks manually with:

```sh
just maintenance::pre-commit
```

The hook reads staged Git index entries, treats `vendor/stim` as a submodule pointer, runs Rust formatting and Clippy only for staged Rust-affecting changes, scans staged source blobs for oversized files, and checks instruction-document structure when `README.md`, `AGENTS.md`, `CLAUDE.md`, or `.gitmodules` changes.
Every scanned `README.md` needs a colocated `AGENTS.md`, and every effective `AGENTS.md` source needs at least one `CLAUDE.md` symlink pointing to it.
