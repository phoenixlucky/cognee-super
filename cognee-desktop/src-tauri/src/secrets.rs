use crate::error::AppError;

const SERVICE: &str = "com.cognee.local.desktop";
pub fn put_secret(name: &str, value: &str) -> Result<(), AppError> {
    keyring::Entry::new(SERVICE, name)
        .map_err(|_| AppError::Secret)?
        .set_password(value)
        .map_err(|_| AppError::Secret)
}
pub fn get_secret(name: &str) -> Result<Option<String>, AppError> {
    match keyring::Entry::new(SERVICE, name)
        .map_err(|_| AppError::Secret)?
        .get_password()
    {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(AppError::Secret),
    }
}
pub fn delete_secret(name: &str) -> Result<(), AppError> {
    match keyring::Entry::new(SERVICE, name)
        .map_err(|_| AppError::Secret)?
        .delete_credential()
    {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(AppError::Secret),
    }
}
