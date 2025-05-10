use error_stack::{AttachmentKind, FrameKind, Report};
use std::fmt::Display;

#[derive(Debug)]
pub enum DMError {
    InvalidData,
    ParserError,
    UiError,
    IOError,
}

impl Display for DMError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            DMError::InvalidData => "Invalid data",
            DMError::ParserError => "Parser error",
            DMError::UiError => "UI error",
            DMError::IOError => "IO error",
        };

        write!(f, "{msg}")
    }
}

impl std::error::Error for DMError {}

pub trait DMErrorExt {
    fn error_str(&self) -> Option<String> {
        None
    }
}

impl DMErrorExt for Report<DMError> {
    fn error_str(&self) -> Option<String> {
        let frame = self.current_frames().last().unwrap();
        if let FrameKind::Attachment(AttachmentKind::Printable(a)) = frame.kind() {
            return Some(a.to_string());
        }

        None
    }
}
