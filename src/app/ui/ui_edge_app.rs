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

use crate::{
    app::{App, ui::normal_block},
    error::DMError,
    mqtt_ctrl::with_mqtt_ctrl,
};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    prelude::{Color, Style},
    text::{Line, Span},
    widgets::{Borders, List, ListItem, Paragraph, Widget},
};

pub fn draw(area: Rect, buf: &mut Buffer, app: &App) -> Result<(), DMError> {
    with_mqtt_ctrl(|mqtt_ctrl| -> Result<(), DMError> {
        let outer_block = normal_block("EdgeApp Management").borders(Borders::NONE);
        let inner_area = outer_block.inner(area);
        outer_block.render(area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([Constraint::Length(5), Constraint::Min(0)].as_ref())
            .split(inner_area);

        // Top: Status info
        {
            let mut status_lines = Vec::<Line>::new();

            if let Some(deployment_status) = mqtt_ctrl.deployment_status() {
                let instances_count = deployment_status.instances().len();
                let modules_count = deployment_status.modules().len();
                let deployment_id = deployment_status
                    .deployment_id()
                    .map(|id| format!("{:?}", id))
                    .unwrap_or_else(|| "none".to_string());
                let reconcile_status = deployment_status.reconcile_status().unwrap_or("none");

                let status_text = format!(
                    "instances={}, modules={}, deployment_id={}, reconcile={}",
                    instances_count, modules_count, deployment_id, reconcile_status
                );
                status_lines.push(Line::from(vec![
                    Span::styled("Deployment Status: ", Style::default().fg(Color::Blue)),
                    Span::styled(status_text, Style::default().fg(Color::White)),
                ]));

                let conditions_met = !deployment_status.instances().is_empty()
                    && !deployment_status.modules().is_empty()
                    && deployment_status.deployment_id().is_some()
                    && reconcile_status == "ok";

                status_lines.push(Line::from(vec![
                    Span::styled("EdgeApp Available: ", Style::default().fg(Color::Blue)),
                    Span::styled(
                        if conditions_met { "Yes" } else { "No" },
                        Style::default().fg(if conditions_met {
                            Color::Green
                        } else {
                            Color::Red
                        }),
                    ),
                ]));
            } else {
                status_lines.push(Line::from(vec![
                    Span::styled("Deployment Status: ", Style::default().fg(Color::Blue)),
                    Span::styled("Not available", Style::default().fg(Color::Red)),
                ]));
            }

            let status_block = normal_block("Status");
            let status_paragraph = Paragraph::new(status_lines)
                .block(status_block)
                .alignment(Alignment::Left);
            status_paragraph.render(chunks[0], buf);
        }

        // Bottom: EdgeApp list
        {
            let mut list_items = Vec::<ListItem>::new();
            let mut available = false;

            // Check if EdgeApp is available based on deployment conditions
            if let Some(deployment_status) = mqtt_ctrl.deployment_status() {
                let conditions_met = !deployment_status.instances().is_empty()
                    && !deployment_status.modules().is_empty()
                    && deployment_status.deployment_id().is_some()
                    && deployment_status.reconcile_status() == Some("ok");

                if conditions_met {
                    available = true;
                    let style = if app.edge_app_list_focus == 0 {
                        Style::default().fg(Color::Black).bg(Color::White)
                    } else {
                        Style::default().fg(Color::White)
                    };

                    list_items.push(ListItem::new(Line::from(vec![Span::styled(
                        "EdgeApp Passthrough",
                        style,
                    )])));
                } else {
                    list_items.push(ListItem::new(Line::from(vec![
                        Span::styled("EdgeApp Passthrough", Style::default().fg(Color::DarkGray)),
                        Span::styled(" (unavailable)", Style::default().fg(Color::Red)),
                    ])));
                }
            } else {
                list_items.push(ListItem::new(Line::from(vec![
                    Span::styled("EdgeApp Passthrough", Style::default().fg(Color::DarkGray)),
                    Span::styled(" (no deployment status)", Style::default().fg(Color::Red)),
                ])));
            }

            if !available {
                list_items.push(ListItem::new(Line::from("")));
                list_items.push(ListItem::new(Line::from(vec![Span::styled(
                    "EdgeApp requires:",
                    Style::default().fg(Color::Yellow),
                )])));
                list_items.push(ListItem::new(Line::from(vec![Span::styled(
                    "• Non-empty instances",
                    Style::default().fg(Color::Gray),
                )])));
                list_items.push(ListItem::new(Line::from(vec![Span::styled(
                    "• Non-empty modules",
                    Style::default().fg(Color::Gray),
                )])));
                list_items.push(ListItem::new(Line::from(vec![Span::styled(
                    "• Valid deployment ID",
                    Style::default().fg(Color::Gray),
                )])));
                list_items.push(ListItem::new(Line::from(vec![Span::styled(
                    "• Reconcile status 'ok'",
                    Style::default().fg(Color::Gray),
                )])));
            }

            let list_block = normal_block("Available EdgeApps");
            Widget::render(List::new(list_items).block(list_block), chunks[1], buf);
        }

        Ok(())
    })
}
