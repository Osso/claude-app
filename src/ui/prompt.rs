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
            class: "prompt-area",
            textarea {
                class: "prompt-input",
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
