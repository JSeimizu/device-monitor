use std::fmt::Display;

#[derive(Debug)]
pub enum DMError {
    InvalidData,
    IOError,
}

impl Display for DMError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            DMError::InvalidData => "Invalid data",
            DMError::IOError => "IO error",
        };

        write!(f, "{msg}")
    }
}

impl std::error::Error for DMError {}
