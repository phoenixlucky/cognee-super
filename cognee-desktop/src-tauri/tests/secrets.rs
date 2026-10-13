use cognee_desktop_lib::{config::AppConfig, secrets};

#[test]
fn credential_store_round_trip_and_config_never_contains_secret_values() {
    let name = format!("test.{}", uuid::Uuid::new_v4());
    let value = format!("private-{}", uuid::Uuid::new_v4());
    secrets::put_secret(&name, &value).expect("write to Windows Credential Manager");
    assert_eq!(
        secrets::get_secret(&name).expect("read credential"),
        Some(value.clone())
    );
    secrets::delete_secret(&name).expect("delete test credential");
    assert_eq!(secrets::get_secret(&name).expect("confirm deletion"), None);

    let config = AppConfig::default();
    let serialized = serde_json::to_string(&config).expect("serialize config");
    assert!(serialized.contains("12315"));
    assert!(!serialized.contains(&value));
    assert!(!serialized.to_ascii_lowercase().contains("api_key"));
    assert!(!serialized.to_ascii_lowercase().contains("password"));
}
