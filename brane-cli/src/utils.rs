use anyhow::Result;
use specifications::errors::SystemDirectoryError;
use std::path::PathBuf;

/* TIM */
/// **Edited: uses dirs_2 instead of appdirs and returns PackageErrors when it goes wrong.**
///
/// Returns the path of the configuration directory. Is guaranteed to exist when it returns successfully.
/// 
/// **Returns**  
/// The path to the brane configuration directory if successful, or a PackageError otherwise.
pub fn get_config_dir() -> Result<PathBuf, SystemDirectoryError> {
    // Try to get the user directory
    let user = match dirs_2::config_dir() {
        Some(user) => user,
        None       => { return Err(SystemDirectoryError::UserConfigDirNotFound); }
    };

    // Check if the brane directory exists and return the path if it does
    let path = user.join("brane");
    if path.exists() { Ok(path) }
    else { Err(SystemDirectoryError::BraneConfigDirNotFound{ path: path }) }
}
/*******/

///
///
///
pub fn uppercase_first_letter(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().chain(c).collect(),
    }
}

///
///
///
pub fn assert_valid_bakery_name(s: &str) -> Result<()> {
    if s.chars().all(|c| c.is_alphanumeric() || c == '_') {
        Ok(())
    } else {
        Err(anyhow!(
            "Invalid name. Must consist only of alphanumeric and/or _ characters."
        ))
    }
}
