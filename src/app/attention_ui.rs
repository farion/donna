use crate::app::ui_style::palette_for;
use crate::attention::{AttentionCandidate, AttentionLevel, AttentionPolicy};
use crate::config::AttentionConfig;
use crate::storage::{AttentionItem, LocalStore};
use eframe::egui::{
    self, Button, Color32, CornerRadius, FontId, Frame, Margin, RichText, Stroke, Vec2,
};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Default)]
pub(super) struct AttentionUiState {
    active: Option<AttentionItem>,
    last_popup_at: Option<i64>,
    last_notified_item_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttentionAction {
    Complete,
    Dismiss,
    SnoozeOneHour,
    NotImportant,
    TodoAlreadyDone,
    TodoLaterToday,
    TodoTomorrow,
    TodoIgnore,
}

impl AttentionUiState {
    pub(super) fn has_active_item(&self) -> bool {
        self.active.is_some()
    }

    pub(super) fn refresh(
        &mut self,
        store: Option<&LocalStore>,
        config: &AttentionConfig,
        ctx: &egui::Context,
        notice: &mut Option<String>,
    ) {
        let Some(store) = store else {
            self.active = None;
            return;
        };
        let Some(now) = unix_now_seconds() else {
            return;
        };

        match store.ready_attention_items(now) {
            Ok(items) => self.set_active_item(items.into_iter().next(), config, ctx, now),
            Err(error) => *notice = Some(error.to_string()),
        }
    }

    pub(super) fn render(
        &mut self,
        ui: &mut egui::Ui,
        store: Option<&LocalStore>,
        notice: &mut Option<String>,
    ) {
        let Some(item) = self.active.clone() else {
            return;
        };

        let mut action = None;
        let palette = palette_for(ui.ctx().theme());
        Frame::NONE
            .fill(palette.attention_card_fill)
            .stroke(egui::Stroke::new(1.0, palette.attention_card_stroke))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.label(
                        RichText::new(attention_label(&item, store))
                            .font(FontId::proportional(11.0))
                            .color(palette.attention_level_text),
                    );
                    ui.label(
                        RichText::new(&item.title)
                            .font(FontId::proportional(15.0))
                            .color(palette.attention_title_text)
                            .strong(),
                    );
                });

                if let Some(body) = &item.body {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(body)
                            .font(FontId::proportional(13.0))
                            .color(palette.attention_body_text),
                    );
                }

                ui.add_space(8.0);
                if item.source_type == "todo_reminder" {
                    action = render_todo_reminder_buttons(ui, &palette);
                } else {
                    ui.horizontal_wrapped(|ui| {
                        if ui.add_sized(button_size(), Button::new("Done")).clicked() {
                            action = Some(AttentionAction::Complete);
                        }
                        if ui
                            .add_sized(button_size(), Button::new("Snooze 1h"))
                            .clicked()
                        {
                            action = Some(AttentionAction::SnoozeOneHour);
                        }
                        if ui
                            .add_sized(button_size(), Button::new("Dismiss"))
                            .clicked()
                        {
                            action = Some(AttentionAction::Dismiss);
                        }
                        if ui
                            .add_sized(button_size(), Button::new("Not important"))
                            .clicked()
                        {
                            action = Some(AttentionAction::NotImportant);
                        }
                    });
                }
            });

        if let Some(action) = action {
            self.apply_action(item.id, action, store, notice);
        }
    }

    #[cfg(test)]
    pub(super) fn dismiss_active(
        &mut self,
        store: Option<&LocalStore>,
        notice: &mut Option<String>,
    ) -> bool {
        let Some(id) = self.active.as_ref().map(|item| item.id) else {
            return false;
        };

        self.apply_action(id, AttentionAction::Dismiss, store, notice);
        true
    }

    fn set_active_item(
        &mut self,
        item: Option<AttentionItem>,
        config: &AttentionConfig,
        ctx: &egui::Context,
        now: i64,
    ) {
        let Some(item) = item else {
            self.active = None;
            self.last_notified_item_id = None;
            return;
        };

        let changed = self.active.as_ref().map(|active| active.id) != Some(item.id);
        self.active = Some(item.clone());

        if !changed || self.last_notified_item_id == Some(item.id) {
            return;
        }

        let level = AttentionLevel::from_name(&item.level).unwrap_or(AttentionLevel::Normal);
        let decision = AttentionPolicy::from_config(config).decide(
            AttentionCandidate {
                level,
                snoozed_until: item.snoozed_until,
            },
            now,
            self.last_popup_at,
        );

        if decision.notify {
            let request_type = match level {
                AttentionLevel::Critical | AttentionLevel::Important => {
                    egui::UserAttentionType::Critical
                }
                AttentionLevel::Normal | AttentionLevel::Info => {
                    egui::UserAttentionType::Informational
                }
            };
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(request_type));
        }

        if decision.raise_window {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            self.last_popup_at = Some(now);
        }

        self.last_notified_item_id = Some(item.id);
    }

    fn apply_action(
        &mut self,
        id: i64,
        action: AttentionAction,
        store: Option<&LocalStore>,
        notice: &mut Option<String>,
    ) {
        let Some(store) = store else {
            *notice = Some("Storage unavailable; cannot update attention item.".to_owned());
            return;
        };

        let result = match action {
            AttentionAction::Complete => store.complete_attention_item(id),
            AttentionAction::Dismiss => store.dismiss_attention_item(id, None),
            AttentionAction::SnoozeOneHour => {
                let Some(now) = unix_now_seconds() else {
                    *notice =
                        Some("System clock unavailable; cannot snooze attention item.".into());
                    return;
                };
                store.snooze_attention_item(id, now + 3_600)
            }
            AttentionAction::NotImportant => {
                store.dismiss_attention_item(id, Some("not_important"))
            }
            AttentionAction::TodoAlreadyDone => {
                if let Some(todo_id) = self.active.as_ref().and_then(|item| item.source_id) {
                    if let Err(error) = store.update_todo_status(todo_id, "done") {
                        *notice = Some(error.to_string());
                        return;
                    }
                }
                store.complete_attention_item(id)
            }
            AttentionAction::TodoLaterToday => {
                self.snooze_todo_reminder(id, store, 4 * 3_600, "later_today", notice)
            }
            AttentionAction::TodoTomorrow => {
                self.snooze_todo_reminder(id, store, 24 * 3_600, "tomorrow", notice)
            }
            AttentionAction::TodoIgnore => {
                self.snooze_todo_reminder(id, store, 3 * 24 * 3_600, "ignore", notice)
            }
        };

        match result {
            Ok(_) => {
                self.active = None;
                self.last_notified_item_id = None;
            }
            Err(error) => *notice = Some(error.to_string()),
        }
    }

    fn snooze_todo_reminder(
        &self,
        id: i64,
        store: &LocalStore,
        seconds: i64,
        feedback: &str,
        notice: &mut Option<String>,
    ) -> Result<AttentionItem, crate::storage::StorageError> {
        let Some(now) = unix_now_seconds() else {
            *notice = Some("System clock unavailable; cannot snooze todo reminder.".into());
            return store.snooze_attention_item(id, 0);
        };
        let until = now + seconds;
        if let Some(todo_id) = self.active.as_ref().and_then(|item| item.source_id) {
            store.snooze_todo_until(todo_id, until)?;
        }
        store.record_attention_feedback(id, feedback)?;
        store.snooze_attention_item(id, until)
    }
}

fn button_size() -> Vec2 {
    Vec2::new(96.0, 30.0)
}

fn render_todo_reminder_buttons(
    ui: &mut egui::Ui,
    palette: &crate::app::ui_style::UiPalette,
) -> Option<AttentionAction> {
    let gap = 8.0;
    let button_width = ((ui.available_width() - gap) / 2.0).max(96.0);
    let button_size = Vec2::new(button_width, 31.0);
    let mut action = None;

    ui.spacing_mut().item_spacing = Vec2::new(gap, 7.0);
    ui.horizontal(|ui| {
        if todo_button(ui, palette, button_size, "Already done").clicked() {
            action = Some(AttentionAction::TodoAlreadyDone);
        }
        if todo_button(ui, palette, button_size, "Later today").clicked() {
            action = Some(AttentionAction::TodoLaterToday);
        }
    });
    ui.horizontal(|ui| {
        if todo_button(ui, palette, button_size, "Tomorrow").clicked() {
            action = Some(AttentionAction::TodoTomorrow);
        }
        if todo_button(ui, palette, button_size, "Ignore").clicked() {
            action = Some(AttentionAction::TodoIgnore);
        }
    });

    action
}

fn attention_label(item: &AttentionItem, store: Option<&LocalStore>) -> String {
    if item.source_type == "todo_reminder" {
        if let Some(severity) = item
            .source_id
            .and_then(|id| store.and_then(|store| store.todo(id).ok()))
            .map(|todo| todo.severity)
        {
            return todo_severity_label(&severity).to_owned();
        }

        return match item.level.as_str() {
            "important" => "HIGH".to_owned(),
            "info" => "LOW".to_owned(),
            _ => "MIDDLE".to_owned(),
        };
    }

    item.level.to_uppercase()
}

fn todo_severity_label(severity: &str) -> &'static str {
    match severity {
        "high" => "HIGH",
        "low" => "LOW",
        _ => "MIDDLE",
    }
}

fn todo_button(
    ui: &mut egui::Ui,
    palette: &crate::app::ui_style::UiPalette,
    size: Vec2,
    text: &str,
) -> egui::Response {
    ui.add_sized(
        size,
        Button::new(
            RichText::new(text)
                .font(FontId::proportional(13.0))
                .color(palette.attention_body_text),
        )
        .fill(attention_button_fill(ui.ctx().theme()))
        .stroke(Stroke::new(1.0, palette.attention_card_stroke))
        .corner_radius(CornerRadius::same(7)),
    )
}

fn attention_button_fill(theme: egui::Theme) -> Color32 {
    match theme {
        egui::Theme::Light => Color32::from_rgb(250, 240, 214),
        egui::Theme::Dark => Color32::from_rgb(61, 53, 44),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{NewAttentionItem, NewTodo};

    #[test]
    fn todo_reminder_label_uses_current_todo_severity() {
        let store = LocalStore::in_memory().expect("store");
        let todo = store
            .create_todo(&NewTodo {
                title: "fix the thing".to_owned(),
                notes: None,
                source: "test".to_owned(),
                related_topic: None,
                severity: "high".to_owned(),
                due_at: None,
            })
            .expect("todo");
        let item = store
            .create_attention_item(&NewAttentionItem {
                source_type: "todo_reminder".to_owned(),
                source_id: Some(todo.id),
                level: "normal".to_owned(),
                title: "Open todo".to_owned(),
                body: Some(todo.title),
                due_at: None,
                payload: None,
            })
            .expect("item");

        assert_eq!(attention_label(&item, Some(&store)), "HIGH");
    }
}

fn unix_now_seconds() -> Option<i64> {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    i64::try_from(seconds).ok()
}
