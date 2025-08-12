#[allow(unused)]
use {
    super::*,
    crate::{
        app::{App, DMScreen},
        azurite::{UiBlob, with_azurite_storage},
        error::DMError,
    },
    chrono::Local,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    error_stack::{Report, Result},
    jlogger_tracing::{JloggerBuilder, LevelFilter, LogTimeFormat, jdebug, jerror, jinfo},
    ratatui::{
        DefaultTerminal, Frame, Terminal,
        buffer::Buffer,
        crossterm::{
            event::{DisableMouseCapture, EnableMouseCapture},
            execute,
            terminal::{
                EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
            },
        },
        layout::{Alignment, Rect},
        layout::{Constraint, Layout},
        prelude::{Backend, CrosstermBackend},
        prelude::{Color, Direction, Style},
        style::Stylize,
        symbols::border,
        text::{Line, Span, Text},
        widgets::{Block, Borders, List, ListItem, Paragraph, Widget},
    },
    std::{
        collections::HashMap,
        io,
        time::{Duration, Instant},
    },
};

#[derive(Debug, Clone)]
pub struct BlobListState {
    pub blobs: Vec<UiBlob>,
    pub selected_index: usize,
    pub container_name: String,
}

impl BlobListState {
    pub fn new(container_name: String) -> Self {
        Self {
            blobs: Vec::new(),
            selected_index: 0,
            container_name,
        }
    }

    pub fn move_up(&mut self) {
        if self.selected_index == 0 {
            self.selected_index = self.blobs.len().saturating_sub(1);
        } else {
            self.selected_index = self.selected_index.saturating_sub(1);
        }
    }

    pub fn move_down(&mut self) {
        if self.selected_index >= self.blobs.len().saturating_sub(1) {
            self.selected_index = 0;
        } else {
            self.selected_index += 1;
        }
    }

    pub fn current_blob(&self) -> Option<&UiBlob> {
        self.blobs.get(self.selected_index)
    }
}

fn format_file_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = size as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", size as u64, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

fn do_list_blobs(blob_state: &BlobListState, area: Rect, buf: &mut Buffer) -> Result<(), DMError> {
    let mut list_items = Vec::<ListItem>::new();

    for (index, blob) in blob_state.blobs.iter().enumerate() {
        let focus = index == blob_state.selected_index;

        // Format: "001  blob_name.txt  2025-08-12T14:03:00Z  1.2 KB"
        let created_str = blob.created_on.to_rfc3339();
        let size_str = format_file_size(blob.size);
        let text = format!(
            "{:3}  {}  {}  {}",
            index + 1,
            blob.name,
            created_str,
            size_str
        );

        list_items_push_text_focus(&mut list_items, &text, focus);
    }

    if list_items.is_empty() {
        list_items.push(ListItem::new(Span::styled(
            "No blobs found in container",
            Style::default().fg(Color::Gray),
        )));
    }

    let title = format!(" Blobs in {} ", blob_state.container_name);
    let block = focus_block(&title);

    List::new(list_items).block(block).render(area, buf);
    Ok(())
}

pub fn draw(area: Rect, buf: &mut Buffer, blob_state: &BlobListState) -> Result<(), DMError> {
    do_list_blobs(blob_state, area, buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_blob_list_state_new() {
        let state = BlobListState::new("test-container".to_string());
        assert_eq!(state.container_name, "test-container");
        assert_eq!(state.selected_index, 0);
        assert!(state.blobs.is_empty());
    }

    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(0), "0 B");
        assert_eq!(format_file_size(512), "512 B");
        assert_eq!(format_file_size(1024), "1.0 KB");
        assert_eq!(format_file_size(1536), "1.5 KB");
        assert_eq!(format_file_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_file_size(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(format_file_size(1024u64.pow(4)), "1.0 TB");
    }

    #[test]
    fn test_blob_list_state_current_blob() {
        let mut state = BlobListState::new("test".to_string());

        // Empty list should return None
        assert!(state.current_blob().is_none());

        // Add a blob
        state.blobs.push(UiBlob {
            name: "test.txt".to_string(),
            created_on: Utc::now(),
            size: 100,
        });

        // Should return the blob
        assert!(state.current_blob().is_some());
        assert_eq!(state.current_blob().unwrap().name, "test.txt");
    }
}
