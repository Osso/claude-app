mod ipc;
mod process;
mod state;
mod ui;
mod watcher;

fn main() {
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
