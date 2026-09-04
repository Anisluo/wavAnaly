use egui::Pos2;
use egui_remixicon::icons;
use serde::{Deserialize, Serialize};

const DEFAULT_SPACE: f32 = 4.;
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CommentMessage {
    pub id: egui::Id,
    pub user: String,
    pub text: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Comment {
    pub id: egui::Id,
    pub rect: egui::Rect,
    pub color: egui::Color32,
    pub offset: Pos2,
    pub anchor: Pos2,
    pub size: Pos2,
    pub annotation_id: egui::Id,
    pub name: String,
    pub visible: bool,
    pub message_id_source: u64,
    pub message_chain: Vec<CommentMessage>,
    pub new_text: String,
    pub save_text: Option<String>,
    pub change: bool,
}

impl Comment {
    pub(crate) fn new(id: egui::Id, annotation_id: egui::Id) -> Self {
        Comment {
            id,
            annotation_id,
            rect: egui::Rect::ZERO,
            color: egui::Color32::WHITE,
            offset: Pos2::ZERO,
            anchor: Pos2::ZERO,
            size: Pos2 { x: 100., y: 50. },
            message_chain: Vec::new(),
            new_text: String::new(),
            name: String::new(),
            visible: false,
            message_id_source: 0,
            save_text: None,
            change: false,
        }
    }
}

impl egui::Widget for &mut Comment {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        // Setup Layout Area
        // We create a temporary rectangle based on the stored position but with a large
        // height to allow the content to expand downwards without clipping during layout.
        let mut layout_rect = self.rect;
        layout_rect.set_height(2000.0);

        // Scope the UI to the specific rectangle so child elements align correctly
        let inner = ui.scope_builder(egui::UiBuilder::new().max_rect(layout_rect), |ui| {
            let line_start = self.rect.left_top();

            // Draw a dashed line from the comment box to the target it's referencing
            ui.painter().add(egui::Shape::dashed_line(
                &[line_start, self.anchor],
                egui::Stroke::new(1.0, egui::Color32::GRAY),
                4.0,
                2.0,
            ));

            // Background Placeholders
            // We reserve slots in the painter order now, but fill them later after
            // we know the final size of the content.
            let background_fill_shape = ui.painter().add(egui::Shape::Noop);
            let background_stroke_shape = ui.painter().add(egui::Shape::Noop);

            // Collapsible State
            // Load whether this specific comment is open or closed from egui's persistent storage
            let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                ui.make_persistent_id(self.id),
                false,
            );

            // Header Logic
            let header = ui.horizontal(|ui| {
                // Custom toggle button using a chat icon instead of the default arrow
                state.show_toggle_button(ui, |ui, _openness, response| {
                    let icon = icons::QUESTION_ANSWER_FILL;
                    ui.painter().text(
                        response.rect.center(),
                        egui::Align2::CENTER_CENTER,
                        icon,
                        egui::FontId::proportional(20.0),
                        ui.visuals().text_color(),
                    );
                });

                // Only show the title label if the header is expanded
                if state.is_open() {
                    ui.add_space(DEFAULT_SPACE);
                    ui.label(egui::RichText::new(&self.name).strong());
                    ui.add_space(DEFAULT_SPACE);
                }
            });

            let mut text_response_rect = egui::Rect::ZERO;

            // Body Logic (Message List & Input)
            state.show_body_unindented(ui, |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = DEFAULT_SPACE;

                    // Top border line
                    ui.painter().add(egui::Shape::line_segment(
                        [ui.cursor().left_top(), ui.cursor().right_top()],
                        egui::Stroke::new(0.5, egui::Color32::WHITE),
                    ));

                    // Render existing comments in the chain
                    for comment in &self.message_chain {
                        ui.label(egui::RichText::new(&comment.text).size(14.0));
                        ui.painter().add(egui::Shape::line_segment(
                            [ui.cursor().left_top(), ui.cursor().right_top()],
                            egui::Stroke::new(0.5, egui::Color32::WHITE),
                        ));
                    }

                    ui.add_space(DEFAULT_SPACE);

                    // Text Input Field
                    let text_response = ui.add(
                        egui::TextEdit::multiline(&mut self.new_text)
                            .desired_rows(1)
                            .desired_width(self.size.x)
                            .hint_text("Comment..."),
                    );

                    // Store rect to prevent dragging the whole widget while typing
                    text_response_rect = text_response.rect;

                    // Handle "Enter" key to submit new comment
                    if text_response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let clean_text = self.new_text.trim().to_string();
                        if !clean_text.is_empty() {
                            self.message_id_source += 1;
                            self.save_text = Some(clean_text);
                            self.change = true;
                        }
                        self.new_text = String::new(); // Clear input after submission
                    }
                });
            });

            // Size Sync
            // Calculate how much space the UI actually took up
            let final_rect = ui.min_rect();
            let content_height = final_rect.height();

            // Auto-expand the saved height if the content grows (adding messages)
            if self.size.y < content_height {
                self.size.y = content_height;
            }

            let background_rect = final_rect.expand(5.0);

            // Delayed Background Rendering
            // Now that we have the final background_rect size, we fill the Noop shapes from earlier
            ui.painter().set(
                background_fill_shape,
                egui::Shape::rect_filled(background_rect, 4.0, egui::Color32::BLACK),
            );
            ui.painter().set(
                background_stroke_shape,
                egui::Shape::rect_stroke(
                    background_rect,
                    4.0,
                    egui::Stroke::new(1.0, egui::Color32::WHITE),
                    egui::StrokeKind::Middle,
                ),
            );

            // TODO: Resize temporary turned off
            // Interaction Logic (Resize & Drag)
            // Define a small interactive handle in the bottom-right corner for resizing
            let handle_rect = egui::Rect::from_min_max(
                background_rect.max - egui::vec2(15.0, 15.0),
                background_rect.max,
            );

            // Logic for resizing the widget (South-East Corner)
            let se_res = ui.interact(handle_rect, self.id.with("se_res"), egui::Sense::drag());

            if se_res.hovered() || se_res.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
            }

            // Draw a small icon to indicate the move handle
            ui.painter().text(
                handle_rect.center(),
                egui::Align2::CENTER_CENTER,
                icons::DRAG_MOVE_2_FILL,
                egui::FontId::proportional(10.0),
                ui.visuals().text_color().linear_multiply(0.4),
            );

            // Logic for moving the entire widget
            // We disable dragging if the user is currently resizing or interacting with the text box
            let can_drag_body = !se_res.dragged()
                && !text_response_rect.contains(ui.ctx().pointer_hover_pos().unwrap_or_default());

            let body_res = ui.interact(
                background_rect,
                self.id.with("body_res"),
                if can_drag_body {
                    egui::Sense::drag()
                } else {
                    egui::Sense::hover()
                },
            );

            if body_res.dragged() {
                let delta = body_res.drag_delta();
                self.offset.x += delta.x;
                self.offset.y += delta.y;
                self.change = true;
            }

            // Logic for Header interactions (Click to toggle, Drag to move)
            let header_res = ui.interact(
                header.response.rect,
                self.id.with("head_res"),
                egui::Sense::click_and_drag(),
            );

            if header_res.clicked() {
                state.toggle(ui);
                state.store(ui.ctx());
            } else if header_res.dragged() {
                self.offset.x += header_res.drag_delta().x;
                self.offset.y += header_res.drag_delta().y;
                self.change = true;
            }

            // Return a combined response so the parent UI knows if any part was touched
            se_res | body_res | header_res
        });

        inner.inner
    }
}
