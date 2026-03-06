use dioxus::prelude::*;

use crate::process;

type Selection = Option<(String, String)>;

fn do_spawn(
    dir: &str,
    task: &str,
    mut selected: Signal<Selection>,
    mut dir_value: Signal<String>,
    mut task_value: Signal<String>,
    mut error_msg: Signal<Option<String>>,
) {
    match process::spawn_orchestrator(dir, task) {
        Ok(_child) => {
            let project_name = project_name_from_dir(dir);
            selected.set(Some((project_name, "manager".to_string())));
            dir_value.set(String::new());
            task_value.set(String::new());
        }
        Err(e) => {
            error_msg.set(Some(format!("{e}")));
        }
    }
}

fn project_name_from_dir(dir: &str) -> String {
    std::path::Path::new(dir)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(dir)
        .to_string()
}

#[component]
fn LaunchFormFields(
    dir_value: Signal<String>,
    task_value: Signal<String>,
    error_msg: Signal<Option<String>>,
    can_submit: bool,
    spawning: Signal<bool>,
    on_start: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "launch-field",
            label { class: "launch-label", "Project directory" }
            input {
                class: "launch-input",
                r#type: "text",
                placeholder: "/path/to/project",
                value: "{dir_value}",
                oninput: move |e| dir_value.clone().set(e.value()),
            }
        }
        div { class: "launch-field",
            label { class: "launch-label", "Task" }
            textarea {
                class: "launch-textarea",
                placeholder: "Describe what the orchestrator should do...",
                value: "{task_value}",
                rows: "4",
                oninput: move |e| task_value.clone().set(e.value()),
            }
        }
        if let Some(err) = error_msg.read().as_ref() {
            div { class: "launch-error", "{err}" }
        }
        button {
            class: if can_submit { "btn btn-primary launch-submit" } else { "btn btn-primary launch-submit launch-submit-disabled" },
            disabled: !can_submit,
            onclick: move |_| on_start.call(()),
            if *spawning.read() { "Starting..." } else { "Start" }
        }
    }
}

#[component]
pub fn LaunchForm() -> Element {
    let selected = use_context::<Signal<Selection>>();
    let dir_value = use_signal(String::new);
    let task_value = use_signal(String::new);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut spawning = use_signal(|| false);

    let can_submit = !dir_value.read().trim().is_empty()
        && !task_value.read().trim().is_empty()
        && !*spawning.read();

    let on_start = move |()| {
        let dir = dir_value.read().trim().to_string();
        let task = task_value.read().trim().to_string();
        if dir.is_empty() || task.is_empty() {
            return;
        }
        spawning.set(true);
        error_msg.set(None);
        do_spawn(&dir, &task, selected, dir_value, task_value, error_msg);
        spawning.set(false);
    };

    rsx! {
        div { class: "launch-form",
            div { class: "launch-form-inner",
                div { class: "launch-heading", "new task" }
                div { class: "launch-subheading", "Spawn an agent-orchestrator run" }
                LaunchFormFields {
                    dir_value, task_value, error_msg,
                    can_submit, spawning,
                    on_start,
                }
            }
        }
    }
}
