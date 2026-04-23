# eos-sys

Low-level (unsafe) Rust FFI bindings to the **Epic Online Services (EOS) C SDK**.

Repository: <https://github.com/devphmd/eos>

## Requirements

This crate **does not** bundle the EOS SDK. You must provide it and point the build to it:

- Set `EOS_SDK_DIR` to the EOS SDK root directory (it must contain `Include/` and `Lib/`).

On Windows you will also need the runtime DLL next to your final executable:
- `EOSSDK-Win64-Shipping.dll`

## No LLVM/clang requirement

By default, `eos-sys` uses **pre-generated bindings** checked into the crate, so **downstream users do not need LLVM/clang (libclang)**.

If you need to regenerate bindings (developer-only):

```bash
cargo clean
cargo build -p eos-sys --no-default-features --features bindgen
```

