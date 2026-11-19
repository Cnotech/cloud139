use crate::client::ClientError;

pub fn invalid(message: impl Into<String>) -> ClientError {
    ClientError::Other(message.into())
}
