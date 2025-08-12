/*
Copyright [2025] Seimizu Joukan

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

use error_stack::{AttachmentKind, FrameKind, Report};
use std::fmt::Display;

#[derive(Debug)]
pub enum DMError {
    InvalidData,
    ParserError,
    UiError,
    IOError,
    RuntimeError,
    Timeout,
}

impl Display for DMError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            DMError::InvalidData => "Invalid data",
            DMError::ParserError => "Parser error",
            DMError::UiError => "UI error",
            DMError::IOError => "IO error",
            DMError::RuntimeError => "Runtime error",
            DMError::Timeout => "Operation timed out",
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
