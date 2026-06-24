#[cfg(test)]
use super::DonnaApp;
use crate::storage::{LocalStore, NewMemory, StoredMemory};
use eframe::egui::{
    self, Button, Color32, CornerRadius, FontId, Frame, Margin, RichText, TextEdit,
};

#[derive(Debug, Default)]
pub(super) struct SensitiveMemoryReviews {
    next_id: u64,
    pending: Vec<PendingSensitiveMemory>,
    stored: Vec<ReviewedSensitiveMemory>,
}

#[derive(Debug, Clone)]
struct PendingSensitiveMemory {
    id: u64,
    draft: NewMemory,
}

#[derive(Debug, Clone)]
struct ReviewedSensitiveMemory {
    memory_id: i64,
    content: String,
    forgotten: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReviewAction {
    SavePending(u64),
    DeletePending(u64),
    UpdateStored(i64),
    ForgetStored(i64),
}

impl SensitiveMemoryReviews {
    pub(super) fn queue(&mut self, memories: Vec<NewMemory>) {
        for memory in memories {
            if !memory.content.trim().is_empty() {
                let id = self.next_id;
                self.next_id += 1;
                self.pending
                    .push(PendingSensitiveMemory { id, draft: memory });
            }
        }
    }

    #[cfg(test)]
    pub(super) fn pending_count(&self) -> usize {
        self.pending.len()
    }

    #[cfg(test)]
    pub(super) fn first_pending_id(&self) -> Option<u64> {
        self.pending.first().map(|memory| memory.id)
    }

    pub(super) fn render(
        &mut self,
        ui: &mut egui::Ui,
        store: Option<&LocalStore>,
        notice: &mut Option<String>,
        chat_width: f32,
    ) {
        if self.pending.is_empty() && self.stored.is_empty() {
            return;
        }

        ui.add_space(6.0);
        ui.label(
            RichText::new("Sensitive memory review")
                .font(FontId::proportional(13.0))
                .color(Color32::from_rgb(78, 64, 54)),
        );

        let mut action = None;
        for pending in &mut self.pending {
            render_pending_memory(ui, chat_width, pending, &mut action);
        }
        for stored in &mut self.stored {
            render_stored_memory(ui, chat_width, stored, &mut action);
        }

        if let Some(action) = action {
            self.apply_action(action, store, notice);
        }
    }

    #[cfg(test)]
    fn correct_pending(&mut self, review_id: u64, content: impl Into<String>) -> bool {
        let Some(index) = self.pending_index(review_id) else {
            return false;
        };

        self.pending[index].draft.content = content.into();
        true
    }

    fn approve_pending(
        &mut self,
        store: Option<&LocalStore>,
        review_id: u64,
    ) -> Result<Option<i64>, String> {
        let Some(index) = self.pending_index(review_id) else {
            return Ok(None);
        };

        let mut draft = self.pending[index].draft.clone();
        draft.content = draft.content.trim().to_owned();
        if draft.content.is_empty() {
            return Ok(None);
        }

        let Some(store) = store else {
            return Err("local storage is unavailable".to_owned());
        };
        let memory = store
            .create_memory(&draft)
            .map_err(|error| error.to_string())?;

        self.pending.remove(index);
        self.stored
            .push(ReviewedSensitiveMemory::from_stored(&memory));
        Ok(Some(memory.id))
    }

    fn delete_pending(&mut self, review_id: u64) -> bool {
        let Some(index) = self.pending_index(review_id) else {
            return false;
        };
        self.pending.remove(index);
        true
    }

    #[cfg(test)]
    fn correct_stored(&mut self, memory_id: i64, content: impl Into<String>) -> bool {
        let Some(index) = self.stored_index(memory_id) else {
            return false;
        };
        self.stored[index].content = content.into();
        true
    }

    fn update_stored(
        &mut self,
        store: Option<&LocalStore>,
        memory_id: i64,
    ) -> Result<Option<i64>, String> {
        let Some(index) = self.stored_index(memory_id) else {
            return Ok(None);
        };
        if self.stored[index].forgotten {
            return Ok(None);
        }

        let content = self.stored[index].content.trim().to_owned();
        if content.is_empty() {
            return Ok(None);
        }

        let Some(store) = store else {
            return Err("local storage is unavailable".to_owned());
        };
        let memory = store
            .update_memory_content(memory_id, &content)
            .map_err(|error| error.to_string())?;

        self.stored[index].content = memory.content;
        Ok(Some(memory.id))
    }

    fn forget_stored(
        &mut self,
        store: Option<&LocalStore>,
        memory_id: i64,
    ) -> Result<Option<i64>, String> {
        let Some(index) = self.stored_index(memory_id) else {
            return Ok(None);
        };

        let Some(store) = store else {
            return Err("local storage is unavailable".to_owned());
        };
        let memory = store
            .forget_memory(memory_id)
            .map_err(|error| error.to_string())?;

        self.stored[index].forgotten = true;
        Ok(Some(memory.id))
    }

    fn apply_action(
        &mut self,
        action: ReviewAction,
        store: Option<&LocalStore>,
        notice: &mut Option<String>,
    ) {
        match action {
            ReviewAction::SavePending(review_id) => match self.approve_pending(store, review_id) {
                Ok(Some(memory_id)) => {
                    *notice = Some(format!(
                        "Approved and saved sensitive memory #{memory_id}. Raw chat was not stored."
                    ));
                }
                Ok(None) => {}
                Err(error) => *notice = Some(error),
            },
            ReviewAction::DeletePending(review_id) => {
                if self.delete_pending(review_id) {
                    *notice = Some(
                        "Deleted the pending sensitive memory. Nothing sensitive was saved."
                            .to_owned(),
                    );
                }
            }
            ReviewAction::UpdateStored(memory_id) => match self.update_stored(store, memory_id) {
                Ok(Some(memory_id)) => {
                    *notice = Some(format!(
                        "Updated sensitive memory #{memory_id}. Raw chat was not stored."
                    ));
                }
                Ok(None) => {}
                Err(error) => *notice = Some(error),
            },
            ReviewAction::ForgetStored(memory_id) => match self.forget_stored(store, memory_id) {
                Ok(Some(memory_id)) => {
                    *notice = Some(format!(
                        "Forgot sensitive memory #{memory_id}. It is no longer active or searchable."
                    ));
                }
                Ok(None) => {}
                Err(error) => *notice = Some(error),
            },
        }
    }

    fn pending_index(&self, review_id: u64) -> Option<usize> {
        self.pending
            .iter()
            .position(|memory| memory.id == review_id)
    }

    fn stored_index(&self, memory_id: i64) -> Option<usize> {
        self.stored
            .iter()
            .position(|memory| memory.memory_id == memory_id)
    }
}

#[cfg(test)]
impl DonnaApp {
    pub(super) fn correct_pending_sensitive_memory(
        &mut self,
        review_id: u64,
        content: impl Into<String>,
    ) -> bool {
        self.sensitive_memory_reviews
            .correct_pending(review_id, content)
    }

    pub(super) fn approve_pending_sensitive_memory(
        &mut self,
        review_id: u64,
    ) -> Result<Option<i64>, String> {
        self.sensitive_memory_reviews
            .approve_pending(self.store.as_ref(), review_id)
    }

    pub(super) fn delete_pending_sensitive_memory(&mut self, review_id: u64) -> bool {
        self.sensitive_memory_reviews.delete_pending(review_id)
    }

    pub(super) fn correct_reviewed_sensitive_memory(
        &mut self,
        memory_id: i64,
        content: impl Into<String>,
    ) -> bool {
        self.sensitive_memory_reviews
            .correct_stored(memory_id, content)
    }

    pub(super) fn update_reviewed_sensitive_memory(
        &mut self,
        memory_id: i64,
    ) -> Result<Option<i64>, String> {
        self.sensitive_memory_reviews
            .update_stored(self.store.as_ref(), memory_id)
    }

    pub(super) fn forget_reviewed_sensitive_memory(
        &mut self,
        memory_id: i64,
    ) -> Result<Option<i64>, String> {
        self.sensitive_memory_reviews
            .forget_stored(self.store.as_ref(), memory_id)
    }
}

impl ReviewedSensitiveMemory {
    fn from_stored(memory: &StoredMemory) -> Self {
        Self {
            memory_id: memory.id,
            content: memory.content.clone(),
            forgotten: memory.forgotten_at.is_some(),
        }
    }
}

fn render_pending_memory(
    ui: &mut egui::Ui,
    chat_width: f32,
    pending: &mut PendingSensitiveMemory,
    action: &mut Option<ReviewAction>,
) {
    ui.push_id(("pending_sensitive_memory", pending.id), |ui| {
        review_frame(ui, |ui| {
            ui.label(
                RichText::new("Pending approval")
                    .font(FontId::proportional(12.0))
                    .color(Color32::from_rgb(117, 81, 55)),
            );
            ui.label(
                RichText::new(format!("Type: {}", pending.draft.memory_type))
                    .font(FontId::proportional(12.0))
                    .color(Color32::from_rgb(89, 96, 103)),
            );
            let edit_width = (chat_width - 52.0).max(180.0);
            ui.add_sized(
                [edit_width, 58.0],
                TextEdit::multiline(&mut pending.draft.content)
                    .desired_rows(2)
                    .desired_width(edit_width),
            );
            ui.horizontal(|ui| {
                let can_save = !pending.draft.content.trim().is_empty();
                if ui
                    .add_enabled(can_save, Button::new("Save"))
                    .on_hover_text("Approve and save this structured memory")
                    .clicked()
                {
                    *action = Some(ReviewAction::SavePending(pending.id));
                }
                if ui
                    .button("Delete")
                    .on_hover_text("Discard this pending memory without saving")
                    .clicked()
                {
                    *action = Some(ReviewAction::DeletePending(pending.id));
                }
            });
        });
    });
}

fn render_stored_memory(
    ui: &mut egui::Ui,
    chat_width: f32,
    stored: &mut ReviewedSensitiveMemory,
    action: &mut Option<ReviewAction>,
) {
    ui.push_id(("stored_sensitive_memory", stored.memory_id), |ui| {
        review_frame(ui, |ui| {
            let status = if stored.forgotten {
                "Forgotten"
            } else {
                "Saved"
            };
            ui.label(
                RichText::new(format!("{status} memory #{}", stored.memory_id))
                    .font(FontId::proportional(12.0))
                    .color(Color32::from_rgb(117, 81, 55)),
            );

            let edit_width = (chat_width - 52.0).max(180.0);
            ui.add_enabled_ui(!stored.forgotten, |ui| {
                ui.add_sized(
                    [edit_width, 58.0],
                    TextEdit::multiline(&mut stored.content)
                        .desired_rows(2)
                        .desired_width(edit_width),
                );
            });

            ui.horizontal(|ui| {
                let can_update = !stored.forgotten && !stored.content.trim().is_empty();
                if ui
                    .add_enabled(can_update, Button::new("Update"))
                    .on_hover_text("Save the corrected memory text")
                    .clicked()
                {
                    *action = Some(ReviewAction::UpdateStored(stored.memory_id));
                }
                if ui
                    .add_enabled(!stored.forgotten, Button::new("Forget"))
                    .on_hover_text("Mark this memory forgotten and remove it from search")
                    .clicked()
                {
                    *action = Some(ReviewAction::ForgetStored(stored.memory_id));
                }
            });
        });
    });
}

fn review_frame(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    Frame::NONE
        .fill(Color32::from_rgb(250, 245, 238))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(223, 207, 189)))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.vertical(add_contents);
        });
}
