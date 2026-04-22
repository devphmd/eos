#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(clippy::all)]

//! Raw FFI bindings for the Epic Online Services (EOS) C SDK.
//!
//! This crate intentionally ships **pre-generated bindings** (checked into source) so
//! downstream users do **not** need LLVM/clang installed.
//!
//! Provide the EOS SDK via the `EOS_SDK_DIR` environment variable.

pub use libc::{c_char, c_double, c_float, c_int, c_longlong, c_short, c_uchar, c_uint, c_ulonglong, c_ushort, size_t};

// By default we ship pre-generated bindings so downstream users don't need LLVM/clang.
// If you need to regenerate: build with `--features eos-sys/bindgen`.
#[cfg(feature = "prebuilt-bindings")]
mod bindings;
#[cfg(feature = "prebuilt-bindings")]
pub use bindings::*;

#[cfg(all(feature = "bindgen", not(feature = "prebuilt-bindings")))]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

