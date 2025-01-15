use lib::{Context, StatementAddress};
use std::cell::LazyCell;
use std::fmt::Write;
use yew::prelude::*;
use yew_hooks::use_local_storage;

fn string_to_html(s: String) -> Html {
    Html::from_html_unchecked(AttrValue::from(s))
}

fn render_inference(ctx: &Context, stmt_addr: StatementAddress) -> String {
    let (hyps, conclusion) = ctx.render_inference(stmt_addr);
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
    let ctx = LazyCell::new(|| Context::load("set.mm", include_bytes!("/tmp/dump/set.mm")));
    let storage = use_local_storage::<String>(String::from("level"));
    let state = use_state(|| ctx.initial_state(storage.as_deref()));
    let current_level_name = ctx.label(state.current_level_stmt_addr);
    if storage.as_deref() != Some(&current_level_name) {
        storage.set(current_level_name.clone());
    }
    let next_level = state.next_level(&ctx);
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
        .buttons(&ctx)
        .into_iter()
        .map(|(stmt_addr, opt_next_state)| {
            let state = state.clone();
            let inference = render_inference(&ctx, stmt_addr);
            let text = string_to_html(format!("{} {inference}", ctx.label(stmt_addr),));
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
        .render_stack(&ctx)
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
                { string_to_html(render_inference(&ctx, state.current_level_stmt_addr)) }
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
