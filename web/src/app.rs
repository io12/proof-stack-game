use lib::{Context, StatementAddress};
use std::fmt::Write;
use std::sync::LazyLock;
use yew::prelude::*;
use yew_hooks::use_local_storage;

static CTX: LazyLock<Context> = LazyLock::new(|| {
    Context::load(
        "set.mm",
        include_bytes!("/tmp/dump/set.mm"),
        lib::TypesetMode::AltHtml,
    )
});

#[export_name = "wizer.initialize"]
pub extern "C" fn force_init() {
    LazyLock::force(&CTX);
}

fn string_to_html(s: String) -> Html {
    Html::from_html_unchecked(AttrValue::from(s))
}

fn render_inference(stmt_addr: StatementAddress) -> String {
    let (hyps, conclusion) = CTX.render_inference(stmt_addr);
    let hyps = hyps.into_iter().fold(String::new(), |mut out, hyp| {
        write!(out, "{hyp} <br/>").unwrap();
        out
    });
    let inference = if hyps.is_empty() {
        conclusion
    } else {
        format!("{hyps} <hr/> {conclusion}")
    };
    format!("<div style='display: inline-block'> {inference} </div>")
}

#[function_component(App)]
pub fn app() -> Html {
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
            let inference = render_inference(stmt_addr);
            let text = string_to_html(format!("{} <br/> <br/> {inference}", CTX.label(stmt_addr)));
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

            let [up_button, down_button, delete_button, copy_button] = [
                ("↑", state.stack_swap(i, i - 1)),
                ("↓", state.stack_swap(i, i + 1)),
                ("🗑️", state.stack_delete(i)),
                ("⿻", state.stack_copy(i)),
            ]
            .map(|(text, next_state)| {
                let (disabled, onclick) = match next_state {
                    Some(next_state) => {
                        let state = state.clone();
                        (
                            false,
                            Some(Callback::from(move |_| state.set(next_state.clone()))),
                        )
                    }
                    None => (true, None),
                };
                html! {
                    <button disabled={disabled} onclick={onclick}>
                        {text}
                    </button>
                }
            });

            html! {
                <li>
                    { up_button }
                    { down_button }
                    { delete_button }
                    { copy_button }
                    { " " }
                    { expr }
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
                <br/>
                { string_to_html(render_inference(state.current_level_stmt_addr)) }
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
