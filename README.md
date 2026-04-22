# eos-sys workspace

This repository is a Cargo workspace containing:

- `crates/eos-sys`: raw FFI bindings to the Epic Online Services C SDK
- `crates/eos-rs`: safe(ish) wrapper on top of `eos-sys`

## Building

Provide the EOS SDK and set:

- `EOS_SDK_DIR` to the EOS SDK root (must contain `Include/` and `Lib/`)

Then:

```bash
cargo build
```

