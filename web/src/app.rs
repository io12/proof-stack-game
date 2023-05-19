use lazy_static::lazy_static;
use lib::Context;
use yew::prelude::*;
use yew_hooks::use_local_storage;

fn string_to_html(s: String) -> Html {
    Html::from_html_unchecked(AttrValue::from(s))
}

#[function_component(App)]
pub fn app() -> Html {
    lazy_static! {
        static ref CTX: Context = Context::load("set.mm", *include_bytes!("/tmp/dump/set.mm"));
    }
    let storage = use_local_storage::<String>(String::from("level"));
    let state = use_state(|| CTX.initial_state(storage.as_deref()));
    let current_level_name = CTX.label(state.current_level_stmt_addr);
    if storage.as_deref() != Some(&current_level_name) {
        storage.set(current_level_name.clone());
    }
    let next_level = state.next_level(&CTX);
    let level_finished = next_level.is_some();
    let next_level_button = {
        let state = state.clone();
        let (disabled, onclick) = match next_level {
            Some(next_state) => (
                false,
                Some(Callback::from(move |_| state.set(next_state.clone()))),
            ),
            None => (true, None),
        };
        html! {
            <button {disabled} {onclick}>
                { "Next level" }
            </button>
        }
    };
    let deps = state
        .buttons(&CTX)
        .into_iter()
        .map(|(stmt_addr, opt_next_state)| {
            let state = state.clone();
            let text = string_to_html(format!(
                "{} {}",
                CTX.label(stmt_addr),
                CTX.render_inference(stmt_addr)
            ));
            let (disabled, onclick) = match opt_next_state {
                Some(next_state) => (
                    level_finished,
                    Some(Callback::from(move |_| state.set(next_state.clone()))),
                ),
                None => (true, None),
            };
            html! {
                <button class="dep-btn" {disabled} {onclick}>
                    { text }
                </button>
            }
        })
        .collect::<Html>();
    let stack = state
        .render_stack(&CTX)
        .into_iter()
        .enumerate()
        .map(|(i, expr)| {
            let expr = string_to_html(format!("<div style='display: inline-block'> {expr} </div>"));

            let up_button = {
                let state = state.clone();
                html! {
                    <button
                        disabled={i == 0}
                        onclick={Callback::from(move |_| state.set(state.stack_swap(i, i - 1)))}
                    >
                        { "↑" }
                    </button>
                }
            };

            let down_button = {
                let state = state.clone();
                html! {
                    <button
                        disabled={i == state.proof_stack.len() - 1}
                        onclick={Callback::from(move |_| state.set(state.stack_swap(i, i + 1)))}
                    >
                        { "↓" }
                    </button>
                }
            };

            html! {
                <li>
                    { expr }
                    { up_button }
                    { down_button }
                </li>
            }
        })
        .collect::<Html>();
    html! {
        <main>
            <h1> { "Proof stack game" } </h1>

            <hr/>

            <h2>
                { "Level" }
                { " " }
                { current_level_name }
                { " " }
                { string_to_html(CTX.render_inference(state.current_level_stmt_addr)) }
                { " " }
                { next_level_button }
            </h2>

            <hr/>

            // Steps to pop onto the stack
            <div class="dep-btns">
                { deps }
            </div>

            <hr/>

            // Proof stack
            <ul>
                { stack }
            </ul>
        </main>
    }
}
