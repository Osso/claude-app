mod api;
mod claude;
mod orchestrator;
mod sandbox;
mod state;
mod ui;
mod worktree;

fn main() {
    tracing_subscriber::fmt::init();

    // Spawn API server on a background thread with its own tokio runtime
    // so it doesn't block the Dioxus event loop
    let project_path = std::env::current_dir().expect("failed to get current directory");
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("failed to create API runtime");
        rt.block_on(api::start_server(project_path));
    });

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
