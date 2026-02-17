mod claude;
mod orchestrator;
mod sandbox;
mod state;
mod ui;
mod worktree;

fn main() {
    tracing_subscriber::fmt::init();
    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            dioxus::desktop::Config::new()
                .with_menu(None)
                .with_window(
                    dioxus::desktop::tao::window::WindowBuilder::new()
                        .with_decorations(false)
                        .with_title("Claude"),
                ),
        )
        .launch(ui::App);
}
