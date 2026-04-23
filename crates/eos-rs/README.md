# eos-rs

Higher-level Rust wrapper over the **Epic Online Services (EOS) C SDK**.

Repository: <https://github.com/devphmd/eos>

This crate depends on [`eos-sys`](../eos-sys/README.md) for the raw bindings.

## Requirements

You must provide the EOS SDK and set:

- `EOS_SDK_DIR` to the EOS SDK root directory (must contain `Include/` and `Lib/`).

On Windows you must ship `EOSSDK-Win64-Shipping.dll` with your application.

