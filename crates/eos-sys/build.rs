use std::env;
use std::path::PathBuf;

// Only needed when `--features eos-sys/bindgen` is enabled.
#[cfg(feature = "bindgen")]
use bindgen;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    println!("cargo:rerun-if-changed={}", manifest_dir.join("wrapper.h").display());
    println!("cargo:rerun-if-env-changed=EOS_SDK_DIR");

    // Link (Windows import lib for EOSSDK-Win64-Shipping.dll).
    //
    // NOTE: We only add link directives if EOS_SDK_DIR is set (recommended for published crates),
    // or if a local ./SDK directory exists (convenient for repo-local development).
    if env::var("CARGO_CFG_TARGET_FAMILY").as_deref() == Ok("windows") {
        let sdk_dir = if let Some(p) = env::var_os("EOS_SDK_DIR") {
            PathBuf::from(p)
        } else {
            // repo-local fallback: <repo>/SDK
            manifest_dir
                .parent()
                .and_then(|p| p.parent())
                .map(|repo_root| repo_root.join("SDK"))
                .unwrap_or_else(|| PathBuf::from("SDK"))
        };

        let lib_dir = sdk_dir.join("Lib");
        if lib_dir.exists() {
            println!("cargo:rerun-if-changed={}", lib_dir.display());
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
            println!("cargo:rustc-link-lib=dylib=EOSSDK-Win64-Shipping");
        } else if env::var_os("EOS_SDK_DIR").is_some() {
            panic!(
                "EOS SDK lib directory not found at {} (from EOS_SDK_DIR).",
                lib_dir.display()
            );
        } else {
            println!(
                "cargo:warning=eos-sys: EOS SDK not found. Set EOS_SDK_DIR to the EOS SDK root (must contain Lib/)."
            );
            // Don't panic here: `cargo package` verification should succeed without the SDK present.
        }
    }

    // Default: no bindgen, no libclang requirement for downstream users.
    // Developer-only: enable `eos-sys/bindgen` feature to regenerate bindings.
    let do_bindgen = env::var_os("CARGO_FEATURE_BINDGEN").is_some();
    let using_prebuilt = env::var_os("CARGO_FEATURE_PREBUILT_BINDINGS").is_some();

    if do_bindgen && using_prebuilt {
        panic!("Features `bindgen` and `prebuilt-bindings` are mutually exclusive");
    }

    #[cfg(feature = "bindgen")]
    {
        if do_bindgen {
            let sdk_dir = if let Some(p) = env::var_os("EOS_SDK_DIR") {
                PathBuf::from(p)
            } else {
                manifest_dir
                    .parent()
                    .and_then(|p| p.parent())
                    .map(|repo_root| repo_root.join("SDK"))
                    .unwrap_or_else(|| PathBuf::from("SDK"))
            };
            let include_dir = sdk_dir.join("Include");

            if !include_dir.exists() {
                panic!(
                    "EOS SDK include directory not found at {}. Set EOS_SDK_DIR to your EOS SDK root.",
                    include_dir.display()
                );
            }
            println!("cargo:rerun-if-changed={}", include_dir.display());

            // Generate bindings from our wrapper header (includes both types and function prototypes).
            let wrapper_header = manifest_dir.join("wrapper.h");
            if !wrapper_header.exists() {
                panic!("wrapper.h not found at {}", wrapper_header.display());
            }

            let clang_include = include_dir.to_string_lossy().to_string();
            let bindings = bindgen::Builder::default()
                .header(wrapper_header.to_string_lossy())
                .clang_arg(format!("-I{clang_include}"))
                .clang_arg("-DEOS_PLATFORM_WINDOWS=1")
                .allowlist_function("EOS_.*")
                .allowlist_type("EOS_.*")
                .allowlist_var("EOS_.*")
                .derive_default(true)
                .derive_debug(true)
                .generate_comments(true)
                .layout_tests(false)
                .generate()
                .expect("Unable to generate EOS bindings");

            let out_path = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
            bindings
                .write_to_file(out_path.join("bindings.rs"))
                .expect("Couldn't write bindings");
        }
    }

    #[cfg(not(feature = "bindgen"))]
    {
        if do_bindgen {
            panic!("Feature `bindgen` requested but eos-sys was built without bindgen support");
        }
    }
}

