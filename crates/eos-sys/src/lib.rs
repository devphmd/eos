#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(clippy::all)]

//! Raw FFI bindings for the Epic Online Services (EOS) C SDK.
//!
//! `eos-sys` is the low-level layer in the EOS Rust stack:
//!
//! - `eos-sys`: raw, mostly 1:1 C bindings
//! - `eos-rs`: higher-level safe wrappers built on top of `eos-sys`
//!
//! # Setup
//!
//! This crate does not bundle the EOS SDK binaries.
//! You must provide the SDK root directory via:
//!
//! - `EOS_SDK_DIR=/path/to/EOS-SDK`
//!
//! The directory must contain:
//!
//! - `Include/`
//! - `Lib/`
//!
//! On Windows, your final application also needs `EOSSDK-Win64-Shipping.dll`
//! next to the executable at runtime.
//!
//! # Why No LLVM Requirement
//!
//! This crate intentionally ships pre-generated bindings in-source, so downstream
//! users do not need LLVM/clang installed to compile.
//!
//! If you maintain bindings and want to regenerate them, build with:
//!
//! `cargo build -p eos-sys --no-default-features --features bindgen`
//!
//! # Safety
//!
//! This crate exposes raw FFI APIs and raw pointers/handles from EOS. Callers are
//! responsible for:
//!
//! - pointer validity,
//! - callback lifetime correctness,
//! - calling the correct EOS `*_Release` APIs.
//!
//! For safer ownership/lifetime behavior, prefer `eos-rs` where possible.

pub use libc::{c_char, c_double, c_float, c_int, c_longlong, c_short, c_uchar, c_uint, c_ulonglong, c_ushort, size_t};

// By default we ship pre-generated bindings so downstream users don't need LLVM/clang.
// If you need to regenerate: build with `--features eos-sys/bindgen`.
#[cfg(feature = "prebuilt-bindings")]
mod bindings;
#[cfg(feature = "prebuilt-bindings")]
pub use bindings::*;

#[cfg(all(feature = "bindgen", not(feature = "prebuilt-bindings")))]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

