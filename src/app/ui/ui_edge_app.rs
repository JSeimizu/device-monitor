use crate::{
    app::{App, ui::normal_block, ui::focus_block},
    error::DMError,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

pub fn draw(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    let edge_apps = app.mqtt_ctrl().edge_app();
    if edge_apps.is_empty() {
        let block = Block::default()
            .title("Edge App")
            .borders(Borders::ALL)
            .border_style(Style::default());
        block.render(area, buf);
        return Ok(());
    }

    if let Some((id, info)) = edge_apps.iter().next() {
        let title = format!("Edge App: {}", id);
        let outer_block = focus_block(&title);

        let inner_area = outer_block.inner(area);
        outer_block.render(area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints(
                [
                    Constraint::Percentage(33),
                    Constraint::Percentage(33),
                    Constraint::Percentage(34),
                ]
                .as_ref(),
            )
            .split(inner_area);


        // Req/Res Info
        let req_res_block = normal_block("Req/Res Info");
        let req_info_text = format!("Request Info: {:#?}", info.module().req_info());
        let res_info_text = format!("Response Info: {:#?}", info.module().res_info());
        let req_res_text = format!("{}\n{}", req_info_text, res_info_text);
        let req_res_paragraph = Paragraph::new(req_res_text).block(req_res_block.clone()).wrap(Wrap { trim: true });
        req_res_paragraph.render(chunks[0], buf);

        // Common Settings
        let common_settings_block = normal_block("Common Settings");
        let common_settings_text = format!("{:#?}", info.module().common_settings());
        let common_settings_paragraph =
            Paragraph::new(common_settings_text).block(common_settings_block.clone()).wrap(Wrap { trim: true });
        common_settings_paragraph.render(chunks[1], buf);

        // Custom Settings
        let custom_settings_block = normal_block("Custom Settings");
        let custom_settings_text = format!("{:#?}", info.module().custom_settings());
        let custom_settings_paragraph =
            Paragraph::new(custom_settings_text).block(custom_settings_block.clone()).wrap(Wrap { trim: true });
        custom_settings_paragraph.render(chunks[2], buf);
    }
    Ok(())
}