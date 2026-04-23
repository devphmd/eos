use std::env;

use eos_rs::{initialize, shutdown, InitializeOptions, Platform, PlatformOptions};

fn required_env(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| format!("missing required environment variable: {name}"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    initialize(InitializeOptions {
        product_name: "eos-rs-example".to_string(),
        product_version: "0.1.0".to_string(),
    })?;

    let platform = Platform::create(PlatformOptions {
        product_id: required_env("EOS_PRODUCT_ID")?,
        sandbox_id: required_env("EOS_SANDBOX_ID")?,
        deployment_id: required_env("EOS_DEPLOYMENT_ID")?,
        client_id: required_env("EOS_CLIENT_ID")?,
        client_secret: required_env("EOS_CLIENT_SECRET")?,
        is_server: false,
        encryption_key: None,
        override_country_code: None,
        override_locale_code: None,
    })?;

    // In a real application, call this from your main loop.
    platform.tick();

    // Optional: access low-level handles for APIs not yet wrapped.
    let _auth = platform.auth().raw_handle();
    let _connect = platform.connect().raw_handle();
    let _lobby = platform.lobby().raw_handle();
    let _p2p = platform.p2p().raw_handle();

    drop(platform);
    shutdown()?;
    println!("eos-rs minimal flow completed.");
    Ok(())
}

