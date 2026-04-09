use dioxus::prelude::*;

use crate::state::Project;

type Selection = Option<(String, String)>;

#[component]
pub fn Sidebar() -> Element {
    let projects = use_context::<Signal<Vec<Project>>>();
    let project_values = projects.read().clone();

    rsx! {
        div { class: "sidebar",
            div { class: "sidebar-header",
                span { "PROJECTS" }
            }
            SidebarList { projects: project_values }
        }
    }
}

#[component]
fn ProjectNode(project: Project) -> Element {
    let mut expanded = use_signal(|| true);
    let project_name = project.name.clone();
    let is_expanded = *expanded.read();
    let icon = expansion_icon(is_expanded);

    rsx! {
        div { class: "project-node",
            ProjectHeader {
                icon,
                project_name,
                on_toggle: move |_| expanded.toggle(),
            }
            ExpandedAgentList {
                expanded: is_expanded,
                project_name: project.name.clone(),
                agents: project.agents.clone(),
            }
        }
    }
}

fn expansion_icon(expanded: bool) -> &'static str {
    if expanded { "\u{25be}" } else { "\u{25b8}" }
}

#[component]
fn ProjectHeader(
    icon: &'static str,
    project_name: String,
    on_toggle: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        div {
            class: "collapsible-header",
            onclick: move |evt| on_toggle.call(evt),
            span { class: "toggle-icon",
                "{icon}"
            }
            span { "{project_name}" }
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

#[component]
fn SidebarList(projects: Vec<Project>) -> Element {
    rsx! {
        div { class: "sidebar-list",
            for project in projects.iter().cloned() {
                ProjectNode { project }
            }
            if projects.is_empty() {
                div { class: "text-sm text-inactive",
                    style: "padding: 8px 12px;",
                    "No projects found"
                }
            }
        }
    }
}

#[component]
fn ExpandedAgentList(expanded: bool, project_name: String, agents: Vec<String>) -> Element {
    if !expanded {
        return rsx! {};
    }
    rsx! {
        div { class: "collapsible-content",
            for agent in agents {
                AgentItem {
                    project_name: project_name.clone(),
                    agent_name: agent,
                }
            }
        }
    }
}
