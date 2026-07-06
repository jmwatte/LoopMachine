use eframe::egui::{self, Color32, RichText};

use crate::app::LoopEditorApp;
use crate::shortcuts::ShortcutAction;

impl LoopEditorApp {
    pub fn show_shortcuts_help(&mut self, ctx: &egui::Context) {
        if !self.show_shortcuts {
            return;
        }
            egui::Window::new("⌨ Toetsenbord Shortcuts")
                .id(egui::Id::new("shortcuts_window"))
                .resizable(true)
                .default_size([400.0, 500.0])
                .default_pos([200.0, 150.0])
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.vertical(|ui| {
                            // ── Dynamische shortcuts uit ShortcutAction::all() ──
                            use crate::shortcuts::ShortcutAction;
                            let categories =
                                ["Playback", "Loop", "Markers", "View", "File", "Edit"];
                            for category in categories {
                                ui.label(
                                    RichText::new(category)
                                        .size(13.0)
                                        .strong()
                                        .color(Color32::from_rgb(180, 180, 220)),
                                );
                                for action in ShortcutAction::all()
                                    .iter()
                                    .filter(|a| a.category() == category)
                                {
                                    let key_text = self
                                        .shortcuts
                                        .binding_for(*action)
                                        .map(|b| b.display())
                                        .unwrap_or_else(|| "—".to_string());
                                    shortcut_row(ui, &key_text, action.display_name());
                                }
                                ui.separator();
                            }

                            // ── Extra muis-acties (geen shortcuts) ──
                            ui.label(
                                RichText::new("Mouse / Interactie")
                                    .size(13.0)
                                    .strong()
                                    .color(Color32::from_rgb(180, 180, 220)),
                            );
                            shortcut_row(ui, "Ctrl+Sleep", "A-B selectie maken");
                            shortcut_row(ui, "Dubbelklik", "Zet A-marker");
                            shortcut_row(ui, "Shift+Dubbelklik", "Zet B-marker");
                            shortcut_row(ui, "Rechterklik", "Wis A-B selectie");
                            shortcut_row(ui, "Scroll", "Zoom in/uit");
                            shortcut_row(ui, "Sleep (geen Ctrl)", "Horizontaal scrollen");
                            ui.separator();

                            if ui.button("⚙ Edit Shortcuts").clicked() {
                                self.show_shortcut_editor = !self.show_shortcut_editor;
                            }
                            ui.label(
                                RichText::new("Druk op F1 om te sluiten")
                                    .size(11.0)
                                    .color(Color32::GRAY),
                            );
                        });
                    });
                });

    }

    pub fn show_shortcut_editor(&mut self, ctx: &egui::Context) {
        if !self.show_shortcut_editor {
            return;
        }
            egui::Window::new("⌨ Shortcut Editor")
                .id(egui::Id::new("shortcut_editor_window"))
                .resizable(true)
                .default_size([550.0, 600.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Klik op een actie en druk op een nieuwe toets om te wijzigen.");
                        if ui.button("🔄 Reset alles naar defaults").clicked() {
                            if let Err(e) = self.shortcuts.reset_all() {
                                self.status_message = format!("Fout: {}", e);
                            } else {
                                self.status_message =
                                    "Alle shortcuts gereset naar defaults".to_string();
                            }
                            self.status_message_timer = 3 * 60;
                        }
                    });
                    ui.separator();

                    // Groepeer per categorie
                    let categories = ["Playback", "Loop", "Markers", "View", "File"];
                    for category in categories {
                        ui.heading(category);
                        for action in ShortcutAction::all()
                            .iter()
                            .filter(|a| a.category() == category)
                        {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(action.display_name()).size(13.0));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("⟲").clicked() {
                                            let _ = self.shortcuts.reset_action(*action);
                                        }

                                        let binding = self
                                            .shortcuts
                                            .binding_for(*action)
                                            .map(|b| b.display())
                                            .unwrap_or_else(|| "—".to_string());

                                        let is_listening =
                                            self.listening_for_action == Some(*action);
                                        let btn_text = if is_listening {
                                            RichText::new("... druk toets ...")
                                                .color(Color32::YELLOW)
                                        } else {
                                            RichText::new(binding)
                                                .color(Color32::from_rgb(200, 200, 60))
                                        };

                                        if ui.button(btn_text).clicked() {
                                            self.listening_for_action = Some(*action);
                                        }
                                    },
                                );
                            });
                        }
                        ui.separator();
                    }
                });

    }
}

fn shortcut_row(ui: &mut egui::Ui, key: &str, description: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(key)
                .size(14.0)
                .strong()
                .color(Color32::from_rgb(200, 200, 60)),
        );
        ui.label(
            RichText::new(description)
                .size(13.0)
                .color(Color32::LIGHT_GRAY),
        );
    });
}
