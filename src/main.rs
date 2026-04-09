mod state;
mod ui;
mod watcher;

fn main() {
    // global_hotkey crate segfaults if DISPLAY is unset on Wayland
    if std::env::var("DISPLAY").unwrap_or_default().is_empty() {
        unsafe { std::env::set_var("DISPLAY", ":0") };
    }

    tracing_subscriber::fmt::init();

    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            dioxus::desktop::Config::new()
                .with_menu(None)
                .with_custom_head(format!(
                    "<style>{}</style>",
                    include_str!("../assets/style.css")
                ))
                .with_window(
                    dioxus::desktop::tao::window::WindowBuilder::new()
                        .with_decorations(false)
                        .with_title("Claude"),
                ),
        )
        .launch(ui::App);
}
