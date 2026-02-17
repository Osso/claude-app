use dioxus::prelude::*;

#[component]
pub fn PromptInput(disabled: bool, on_submit: EventHandler<String>) -> Element {
    let mut text = use_signal(String::new);

    let mut submit = move || {
        let value = text.read().trim().to_string();
        if !value.is_empty() {
            on_submit.call(value);
            text.set(String::new());
        }
    };

    rsx! {
        div {
            style: "padding: 8px; border-top: 1px solid #333;",
            textarea {
                style: "width: 100%; min-height: 60px; max-height: 200px; background: #252535; color: #e0e0e0; border: 1px solid #444; border-radius: 4px; padding: 8px; font-family: monospace; font-size: 0.95em; resize: vertical; box-sizing: border-box;",
                disabled: disabled,
                placeholder: if disabled { "Waiting for response..." } else { "Type a message... (Enter to send, Shift+Enter for newline)" },
                value: "{text}",
                oninput: move |evt| text.set(evt.value()),
                onkeydown: move |evt: KeyboardEvent| {
                    if evt.key() == Key::Enter && !evt.modifiers().shift() {
                        evt.prevent_default();
                        submit();
                    }
                },
            }
        }
    }
}
