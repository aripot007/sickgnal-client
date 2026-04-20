slint::include_modules!();

pub mod callbacks;
pub mod converters;
pub mod events;
pub mod sdk_runner;
pub mod ui_helpers;

// Android entry point lives in the lib
#[cfg(feature = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    use sickgnal_sdk::TlsConfig;
    use sickgnal_sdk::account::ProfileManager;
    use std::path::PathBuf;
    use std::sync::Arc;

    slint::android::init(app).unwrap();

    let base_dir: PathBuf = "storage".into();
    tracing_subscriber::fmt::init();

    let rt = Arc::new(tokio::runtime::Runtime::new().expect("Failed to create tokio runtime"));
    let ui = AppWindow::new().expect("Failed to load UI");

    ui.global::<Auth>().set_tls_warning("".into());

    let profile_manager = ProfileManager::new(base_dir.clone()).expect("create profile manager");
    let profiles = profile_manager.list_profiles().unwrap_or_default();

    callbacks::no_sdk::setup_callbacks_no_sdk(&ui);
    callbacks::before_sdk::setup_callbacks_before_sdk(&ui);
    callbacks::auth::setup_callbacks_auth(
        &ui,
        Arc::clone(&rt),
        profile_manager,
        profiles,
        base_dir,
        "sickgnal.bapttf.com:443".into(),
        TlsConfig::Rustls { custom_ca: None },
    );

    ui.run().unwrap();
}
