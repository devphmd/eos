use std::env;
use std::ffi::CString;
use std::ptr::{null, null_mut};

fn required_env(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| format!("missing required environment variable: {name}"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Required EOS values. In real apps, load these from secure configuration.
    let product_id = CString::new(required_env("EOS_PRODUCT_ID")?)?;
    let sandbox_id = CString::new(required_env("EOS_SANDBOX_ID")?)?;
    let deployment_id = CString::new(required_env("EOS_DEPLOYMENT_ID")?)?;
    let client_id = CString::new(required_env("EOS_CLIENT_ID")?)?;
    let client_secret = CString::new(required_env("EOS_CLIENT_SECRET")?)?;

    let product_name = CString::new("eos-sys-example")?;
    let product_version = CString::new("0.1.0")?;

    let init_opts = eos_sys::EOS_InitializeOptions {
        ApiVersion: eos_sys::EOS_INITIALIZE_API_LATEST as i32,
        AllocateMemoryFunction: None,
        ReallocateMemoryFunction: None,
        ReleaseMemoryFunction: None,
        ProductName: product_name.as_ptr(),
        ProductVersion: product_version.as_ptr(),
        Reserved: null_mut(),
        SystemInitializeOptions: null_mut(),
        OverrideThreadAffinity: null_mut(),
    };

    unsafe {
        let res = eos_sys::EOS_Initialize(&init_opts);
        if res != eos_sys::EOS_EResult_EOS_Success {
            return Err(format!("EOS_Initialize failed with code {res}").into());
        }
    }

    let platform_opts = eos_sys::EOS_Platform_Options {
        ApiVersion: eos_sys::EOS_PLATFORM_OPTIONS_API_LATEST as i32,
        Reserved: null_mut(),
        ProductId: product_id.as_ptr(),
        SandboxId: sandbox_id.as_ptr(),
        ClientCredentials: eos_sys::EOS_Platform_ClientCredentials {
            ClientId: client_id.as_ptr(),
            ClientSecret: client_secret.as_ptr(),
        },
        bIsServer: 0,
        EncryptionKey: null(),
        OverrideCountryCode: null(),
        OverrideLocaleCode: null(),
        DeploymentId: deployment_id.as_ptr(),
        Flags: 0,
        CacheDirectory: null(),
        TickBudgetInMilliseconds: 0,
        RTCOptions: null(),
        IntegratedPlatformOptionsContainerHandle: null_mut(),
        SystemSpecificOptions: null(),
        TaskNetworkTimeoutSeconds: null_mut(),
    };

    unsafe {
        let platform = eos_sys::EOS_Platform_Create(&platform_opts);
        if platform.is_null() {
            let _ = eos_sys::EOS_Shutdown();
            return Err("EOS_Platform_Create returned null".into());
        }

        // In a game/app loop this should be called regularly.
        eos_sys::EOS_Platform_Tick(platform);

        eos_sys::EOS_Platform_Release(platform);

        let shutdown = eos_sys::EOS_Shutdown();
        if shutdown != eos_sys::EOS_EResult_EOS_Success {
            return Err(format!("EOS_Shutdown failed with code {shutdown}").into());
        }
    }

    println!("EOS platform minimal flow completed.");
    Ok(())
}

