use dioxus::prelude::*;

use crate::state::Project;

type Selection = Option<(String, String)>;

#[component]
pub fn Sidebar() -> Element {
    let projects = use_context::<Signal<Vec<Project>>>();

    rsx! {
        div { class: "sidebar",
            div { class: "sidebar-header",
                span { "PROJECTS" }
                NewTaskButton {}
            }
            div { class: "sidebar-list",
                for project in projects.read().iter() {
                    ProjectNode { project: project.clone() }
                }
                if projects.read().is_empty() {
                    div { class: "text-sm text-inactive",
                        style: "padding: 8px 12px;",
                        "No projects found"
                    }
                }
            }
        }
    }
}

#[component]
fn ProjectNode(project: Project) -> Element {
    let mut expanded = use_signal(|| true);
    let project_name = project.name.clone();

    rsx! {
        div { class: "project-node",
            div {
                class: "collapsible-header",
                onclick: move |_| expanded.toggle(),
                span { class: "toggle-icon",
                    if *expanded.read() { "\u{25be}" } else { "\u{25b8}" }
                }
                span { "{project_name}" }
            }
            if *expanded.read() {
                div { class: "collapsible-content",
                    for agent in project.agents.iter() {
                        AgentItem {
                            project_name: project.name.clone(),
                            agent_name: agent.clone(),
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn AgentItem(project_name: String, agent_name: String) -> Element {
    let mut selected = use_context::<Signal<Selection>>();
    let is_active = selected
        .read()
        .as_ref()
        .map(|(p, a)| p == &project_name && a == &agent_name)
        .unwrap_or(false);

    let pn = project_name.clone();
    let an = agent_name.clone();
    let badge = role_badge(&agent_name);

    rsx! {
        div {
            class: if is_active { "agent-entry active" } else { "agent-entry" },
            onclick: move |_| selected.set(Some((pn.clone(), an.clone()))),
            span { class: "agent-entry-label", "{agent_name}" }
            span { class: "badge badge-idle", "{badge}" }
        }
    }
}

#[component]
fn NewTaskButton() -> Element {
    let mut selected = use_context::<Signal<Selection>>();

    rsx! {
        button {
            class: "btn-new-task",
            title: "New task",
            onclick: move |_| selected.set(None),
            "+"
        }
    }
}

fn role_badge(name: &str) -> &str {
    if name.starts_with("developer") {
        "dev"
    } else if name == "manager" {
        "mgr"
    } else if name == "architect" {
        "arch"
    } else if name == "scorer" {
        "scr"
    } else {
        ""
    }
}
