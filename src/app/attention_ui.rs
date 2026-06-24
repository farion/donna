use crate::attention::{AttentionCandidate, AttentionLevel, AttentionPolicy};
use crate::config::AttentionConfig;
use crate::storage::{AttentionItem, LocalStore};
use eframe::egui::{self, Button, Color32, CornerRadius, FontId, Frame, Margin, RichText, Vec2};
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
        Frame::NONE
            .fill(Color32::from_rgb(255, 248, 224))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(item.level.to_uppercase())
                            .font(FontId::proportional(11.0))
                            .color(Color32::from_rgb(127, 83, 26)),
                    );
                    ui.label(
                        RichText::new(&item.title)
                            .font(FontId::proportional(15.0))
                            .strong(),
                    );
                });

                if let Some(body) = &item.body {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(body)
                            .font(FontId::proportional(13.0))
                            .color(Color32::from_rgb(72, 83, 92)),
                    );
                }

                ui.add_space(8.0);
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
        };

        match result {
            Ok(_) => {
                self.active = None;
                self.last_notified_item_id = None;
            }
            Err(error) => *notice = Some(error.to_string()),
        }
    }
}

fn button_size() -> Vec2 {
    Vec2::new(96.0, 30.0)
}

fn unix_now_seconds() -> Option<i64> {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    i64::try_from(seconds).ok()
}
