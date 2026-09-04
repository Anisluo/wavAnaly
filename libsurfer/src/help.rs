//! Help texts and dialogs.
use egui::{Context, Grid, OpenUrl, RichText, ScrollArea, Ui, Window};
use egui_remixicon::icons;
use emath::{Align2, Pos2};

use crate::keyboard_shortcuts::{ShortcutAction, SurferShortcuts};
use crate::wave_source::LoadOptions;
use crate::{SystemState, message::Message};

impl SystemState {
    pub fn help_message(&self, ui: &mut Ui) {
        if self.user.waves.is_none() {
            let show_command_prompt = self
                .user
                .config
                .shortcuts
                .format_shortcut(ShortcutAction::ShowCommandPrompt);

            ui.label(RichText::new(
                t!("Drag and drop a VCD, FST, or GHW file here to open it"),
            ));

            #[cfg(target_arch = "wasm32")]
            ui.label(RichText::new(
                t!("Or press {} and type load_url").replacen("{}", &show_command_prompt, 1),
            ));
            #[cfg(not(target_arch = "wasm32"))]
            ui.label(RichText::new(
                t!("Or press {} and type load_file or load_url")
                    .replacen("{}", &show_command_prompt, 1),
            ));
            #[cfg(target_arch = "wasm32")]
            ui.label(RichText::new(
                t!("Or use the file menu or toolbar to open a URL"),
            ));
            #[cfg(not(target_arch = "wasm32"))]
            ui.label(RichText::new(
                t!("Or use the file menu or toolbar to open a file or a URL"),
            ));
            ui.horizontal(|ui| {
                ui.label(RichText::new(t!("Or click")));
                if ui.link(t!("here")).clicked() {
                    self.channels
                        .msg_sender
                        .send(Message::LoadWaveformFileFromUrl(
                            "https://app.surfer-project.org/picorv32.vcd".to_string(),
                            LoadOptions::Clear,
                        ))
                        .ok();
                }
                ui.label(t!("to open an example waveform"));
            });

            #[cfg(not(test))]
            if !self.file_history.files().is_empty() {
                ui.add_space(10.0);
                ui.label(RichText::new(t!("Recent files")));

                let labels = self.file_history.display_labels();
                for (path, label) in self.file_history.files().iter().zip(labels.iter()) {
                    if ui.link(label).on_hover_text(path.as_str()).clicked() {
                        self.channels
                            .msg_sender
                            .send(Message::LoadFile(path.clone(), LoadOptions::Clear))
                            .ok();
                    }
                }
            }

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(20.0);
        }

        controls_listing(ui, &self.user.config.shortcuts);

        ui.add_space(20.0);
        ui.separator();
        ui.add_space(20.0);

        #[cfg(target_arch = "wasm32")]
        {
            ui.label(RichText::new(
            t!("Note that this web based version is a bit slower than a natively installed version. There may also be a long delay with unresponsiveness when loading large waveforms because the web assembly version does not currently support multi threading."),
        ));

            ui.hyperlink_to(
                "See https://gitlab.com/surfer-project/surfer for install instructions",
                "https://gitlab.com/surfer-project/surfer",
            );
        }
    }
}

pub fn draw_about_window(ctx: &Context, msgs: &mut Vec<Message>) {
    let mut open = true;
    Window::new(t!("About wavAnaly"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(t!("🏄 wavAnaly")).monospace().size(24.));
                ui.add_space(20.);
                ui.label(
                    t!("Cargo version: {}")
                        .replacen("{}", env!("CARGO_PKG_VERSION"), 1),
                );
                if ui
                    .small_button(
                        t!("Git version: {}")
                            .replacen("{}", env!("VERGEN_GIT_DESCRIBE"), 1),
                    )
                    .on_hover_text(t!("Click to copy git version"))
                    .clicked()
                {
                    ctx.copy_text(env!("VERGEN_GIT_DESCRIBE").to_string());
                }
                ui.label(
                    t!("Build date: {}")
                        .replacen("{}", env!("VERGEN_BUILD_DATE"), 1),
                );
                ui.hyperlink_to(
                    (icons::GITLAB_FILL).to_string() + t!(" repository"),
                    "https://gitlab.com/surfer-project/surfer",
                );
                ui.hyperlink_to(t!("Homepage"), "https://surfer-project.org/");
                ui.add_space(10.);
                if ui.button(t!("Close")).clicked() {
                    msgs.push(Message::SetAboutVisible(false));
                }
            })
        });
    if !open {
        msgs.push(Message::SetAboutVisible(false));
    }
}

pub fn draw_quickstart_help_window(
    ctx: &Context,
    msgs: &mut Vec<Message>,
    shortcuts: &SurferShortcuts,
) {
    let mut open = true;
    let show_command_prompt = shortcuts.format_shortcut(ShortcutAction::ShowCommandPrompt);
    Window::new(t!("🏄 wavAnaly quick start"))
        .collapsible(true)
        .resizable(true)
        .pivot(Align2::CENTER_CENTER)
        .open(&mut open)
        .default_pos(Pos2::new(
            ctx.content_rect().size().x * 0.5,
            ctx.content_rect().size().y * 0.5,
        ))
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.add_space(5.);

                ui.label(RichText::new(t!("Controls")).size(20.));
                ui.add_space(5.);
                ui.label(t!("↔ Use scroll and ctrl+scroll to navigate the waveform"));
                ui.label(
                    t!("🚀 Press {} to open the command palette")
                        .replacen("{}", &show_command_prompt, 1),
                );
                ui.label(t!("✋ Click the middle mouse button for gestures"));
                ui.label(t!("❓ See the help menu for more controls"));
                ui.add_space(10.);
                ui.label(RichText::new(t!("Adding traces")).size(20.));
                ui.add_space(5.);
                ui.label(t!("Add more traces using the command palette or using the sidebar"));
                ui.add_space(10.);
                ui.label(RichText::new(t!("Opening files")).size(20.));
                ui.add_space(5.);
                ui.label(t!("Open a new file by"));
                ui.label(t!("- dragging a VCD, FST, or GHW file"));
                #[cfg(target_arch = "wasm32")]
                ui.label(t!("- typing load_url in the command palette"));
                #[cfg(not(target_arch = "wasm32"))]
                ui.label(t!("- typing load_url or load_file in the command palette"));
                ui.label(t!("- using the file menu"));
                ui.label(t!("- using the toolbar"));
                ui.add_space(10.);
            });
            ui.vertical_centered(|ui| {
                if ui.button(t!("Close")).clicked() {
                    msgs.push(Message::SetQuickStartVisible(false));
                }
            })
        });
    if !open {
        msgs.push(Message::SetQuickStartVisible(false));
    }
}

pub fn draw_control_help_window(
    ctx: &Context,
    msgs: &mut Vec<Message>,
    shortcuts: &SurferShortcuts,
) {
    let mut open = true;
    Window::new(t!("🖮 wavAnaly controls"))
        .collapsible(true)
        .resizable(true)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                key_listing(ui, shortcuts);
                ui.add_space(10.);
                if ui.button(t!("Close")).clicked() {
                    msgs.push(Message::SetKeyHelpVisible(false));
                }
            });
        });
    if !open {
        msgs.push(Message::SetKeyHelpVisible(false));
    }
}

/// Long list of key binding for the dialog.
fn key_listing(ui: &mut Ui, shortcuts: &SurferShortcuts) {
    let save_state_file = shortcuts.format_shortcut(ShortcutAction::SaveStateFile);
    let toggle_hierarchy = shortcuts.format_shortcut(ShortcutAction::ToggleSidePanel);
    let toggle_toolbar = shortcuts.format_shortcut(ShortcutAction::ToggleToolbar);
    let reload_waveform = shortcuts.format_shortcut(ShortcutAction::ReloadWaveform);
    let focus_item = shortcuts.format_shortcut(ShortcutAction::ItemFocus);
    let goto_end = shortcuts.format_shortcut(ShortcutAction::GoToEnd);
    let goto_start = shortcuts.format_shortcut(ShortcutAction::GoToStart);
    let zoom_in = shortcuts.format_shortcut(ShortcutAction::ZoomIn);
    let zoom_out = shortcuts.format_shortcut(ShortcutAction::ZoomOut);
    let show_command_prompt = shortcuts.format_shortcut(ShortcutAction::ShowCommandPrompt);
    let selected_item_toggle = shortcuts.format_shortcut(ShortcutAction::SelectToggle);
    let undo = shortcuts.format_shortcut(ShortcutAction::Undo);
    let redo = shortcuts.format_shortcut(ShortcutAction::Redo);
    let add_marker = shortcuts.format_shortcut(ShortcutAction::MarkerAdd);
    let scroll_up = shortcuts.format_shortcut(ShortcutAction::ScrollUp);
    let scroll_down = shortcuts.format_shortcut(ShortcutAction::ScrollDown);
    let delete_selected = shortcuts.format_shortcut(ShortcutAction::DeleteSelected);
    let toggle_menu = shortcuts.format_shortcut(ShortcutAction::ToggleMenu);
    let divider_add = shortcuts.format_shortcut(ShortcutAction::DividerAdd);
    let zoom_to_cursor = shortcuts.format_shortcut(ShortcutAction::ZoomToCursor);
    #[cfg(not(target_arch = "wasm32"))]
    let ui_zoom_in = shortcuts.format_shortcut(ShortcutAction::UiZoomIn);
    #[cfg(not(target_arch = "wasm32"))]
    let ui_zoom_out = shortcuts.format_shortcut(ShortcutAction::UiZoomOut);
    let keys = vec![
        ("🚀", show_command_prompt.as_str(), t!("Show command prompt")),
        ("↔", "Scroll", t!("Pan")),
        ("🔎", "Ctrl+Scroll", t!("Zoom")),
        (icons::SAVE_FILL, &save_state_file, t!("Save the state")),
        (
            icons::LAYOUT_LEFT_FILL,
            &toggle_hierarchy,
            t!("Show or hide the design hierarchy"),
        ),
        (icons::MENU_FILL, &toggle_menu, t!("Show or hide menu")),
        (icons::TOOLS_FILL, &toggle_toolbar, t!("Show or hide toolbar")),
        (icons::ZOOM_IN_FILL, &zoom_in, t!("Zoom in")),
        (icons::ZOOM_OUT_FILL, &zoom_out, t!("Zoom out")),
        (icons::TARGET_FILL, &zoom_to_cursor, t!("Zoom in on cursor")),
        #[cfg(not(target_arch = "wasm32"))]
        ("", &ui_zoom_in, t!("UI Zoom in")),
        #[cfg(not(target_arch = "wasm32"))]
        ("", &ui_zoom_out, t!("UI Zoom out")),
        ("", "k/⬆", t!("Scroll up")),
        ("", "j/⬇", t!("Scroll down")),
        ("", "Ctrl+k/⬆", t!("Move focused item up")),
        ("", "Ctrl+j/⬇", t!("Move focused item down")),
        ("", "Alt+k/⬆", t!("Move focus up")),
        ("", "Alt+j/⬇", t!("Move focus down")),
        ("", &selected_item_toggle, t!("Add focused item to selection")),
        ("", "Ctrl+Alt+k/⬆", t!("Extend selection up")),
        ("", "Ctrl+Alt+j/⬇", t!("Extend selection down")),
        ("", &undo, t!("Undo last change")),
        ("", &redo, t!("Redo last change")),
        ("", &focus_item, t!("Fast focus a variable")),
        ("", &add_marker, t!("Add marker at current cursor")),
        ("", "Ctrl+0-9", t!("Add numbered marker")),
        ("", "0-9", t!("Center view at numbered marker")),
        ("", &divider_add, t!("Add divider")),
        (icons::REWIND_START_FILL, &goto_start, t!("Go to start")),
        (icons::FORWARD_END_FILL, &goto_end, t!("Go to end")),
        (icons::REFRESH_LINE, &reload_waveform, t!("Reload waveform")),
        (icons::SPEED_FILL, &scroll_up, t!("Go one page/screen right")),
        (icons::REWIND_FILL, &scroll_down, t!("Go one page/screen left")),
        (
            icons::PLAY_FILL,
            "➡/l",
            t!("Go to next transition of focused variable (changeable in config)"),
        ),
        (
            icons::PLAY_REVERSE_FILL,
            "⬅/h",
            t!("Go to previous transition of focused variable (changeable in config)"),
        ),
        (
            "",
            "Ctrl+➡/l",
            t!("Go to next non-zero transition of focused variable"),
        ),
        (
            "",
            "Ctrl+⬅/h",
            t!("Go to previous non-zero transition of focused variable"),
        ),
        (
            icons::DELETE_BIN_2_FILL,
            &delete_selected,
            t!("Delete focused item"),
        ),
        #[cfg(not(target_arch = "wasm32"))]
        (icons::FULLSCREEN_LINE, "F11", t!("Toggle full screen")),
    ];

    Grid::new("keys")
        .num_columns(3)
        .spacing([5., 5.])
        .show(ui, |ui| {
            for (symbol, control, description) in keys {
                let control = ctrl_to_cmd(control);
                ui.label(symbol);
                ui.label(control);
                ui.label(description);
                ui.end_row();
            }
        });

    add_hint_text(ui);
}

/// Shorter list displayed at startup screen.
fn controls_listing(ui: &mut Ui, shortcuts: &SurferShortcuts) {
    let show_command_prompt = shortcuts.format_shortcut(ShortcutAction::ShowCommandPrompt);
    let toggle_hierarchy = shortcuts.format_shortcut(ShortcutAction::ToggleSidePanel);
    let toggle_toolbar = shortcuts.format_shortcut(ShortcutAction::ToggleToolbar);
    let toggle_menu = shortcuts.format_shortcut(ShortcutAction::ToggleMenu);

    let controls = vec![
        ("🚀", show_command_prompt.as_str(), t!("Show command prompt")),
        ("↔", "Horizontal Scroll", t!("Pan")),
        ("↕", "j, k, Up, Down", t!("Scroll down/up")),
        ("⌖", "Ctrl+j, k, Up, Down", t!("Move focus down/up")),
        ("🔃", "Alt+j, k, Up, Down", t!("Move focused item down/up")),
        ("🔎", "Ctrl+Scroll", t!("Zoom")),
        (
            icons::LAYOUT_LEFT_2_FILL,
            &toggle_hierarchy,
            t!("Show or hide the design hierarchy"),
        ),
        (icons::MENU_FILL, &toggle_menu, t!("Show or hide menu")),
        (icons::TOOLS_FILL, &toggle_toolbar, t!("Show or hide toolbar")),
    ];

    Grid::new("controls")
        .num_columns(2)
        .spacing([20., 5.])
        .show(ui, |ui| {
            for (symbol, control, description) in controls {
                let control = ctrl_to_cmd(control);
                ui.label(format!("{symbol}  {control}"));
                ui.label(description);
                ui.end_row();
            }
        });
    add_hint_text(ui);
}

fn add_hint_text(ui: &mut Ui) {
    ui.add_space(20.);
    ui.label(RichText::new(t!("Hint: You can repeat keybinds by typing Alt+0-9 before them. For example, Alt+1 Alt+0 k scrolls 10 steps up.")));
}

// Display information about licenses for wavAnaly and used crates.
pub fn draw_license_window(ctx: &Context, msgs: &mut Vec<Message>) {
    let mut open = true;
    let text = include_str!("../../LICENSE-EUPL-1.2.txt");
    Window::new(t!("wavAnaly License"))
        .open(&mut open)
        .collapsible(false)
        .max_height(600.)
        .default_size((600., 600.))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                ui.label(text);
            });
            ui.add_space(10.);
            ui.horizontal(|ui| {
                if ui.button(t!("Dependency licenses")).clicked() {
                    ctx.open_url(OpenUrl {
                        url: "https://docs.surfer-project.org/licenses.html".to_string(),
                        new_tab: true,
                    });
                }
                if ui.button(t!("Close")).clicked() {
                    msgs.push(Message::SetLicenseVisible(false));
                }
            });
        });
    if !open {
        msgs.push(Message::SetLicenseVisible(false));
    }
}

// Replace Ctrl with Cmd in case of macos, unless we are running tests
fn ctrl_to_cmd(instr: &str) -> String {
    #[cfg(all(target_os = "macos", not(test)))]
    let instring = instr.to_string().replace("Ctrl", "Cmd");
    #[cfg(any(not(target_os = "macos"), test))]
    let instring = instr.to_string();
    instring
}
