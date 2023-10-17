use anyhow::{anyhow, Error};
use std::fmt::Display;

/// Map Mutex Error to Anyhow Error
///
/// This function takes an error of a type that implements the `Display` trait and maps it to an
/// `anyhow::Error`. It is commonly used to convert errors that occur when attempting to lock a
/// mutex into more detailed and user-friendly error messages.
///
/// # Arguments
///
/// * `err`: An error of a type that implements the `Display` trait, typically the error returned
/// from a mutex lock operation.
///
/// # Returns
///
/// Returns an `anyhow::Error` that encapsulates the original error message along with additional
/// information regarding the mutex lock failure.
///
/// # Example
///
/// ```rust
/// use std::sync::Mutex;
/// use anyhow::Error;
/// use your_module::map_mutex_err;
///
/// fn try_lock_mutex(mutex: &Mutex<i32>) -> Result<(), Error> {
///     mutex.lock()
///         .map_err(|err| map_mutex_err(err)) // Convert Mutex error to Anyhow Error
///         .map(|guard| {
///             // Perform operations with the mutex guard
///         })
/// }
/// ```
///
/// In this example, the `map_mutex_err` function is used to convert a `Mutex` lock error into
/// an `anyhow::Error`, providing more detailed error information to the caller.
pub fn map_mutex_err<T: Display>(err: T) -> Error {
    anyhow!("Lock mutex failed: {}", err)
}
