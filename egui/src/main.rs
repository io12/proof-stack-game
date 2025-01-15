use eframe::egui;
use memoize::memoize;
use std::fmt::Write;

#[memoize]
fn tex_to_svg(tex: String) -> Vec<u8> {
    mathjax_svg::convert_to_svg(tex).unwrap().into_bytes()
}

fn tex_to_image(tex: String) -> egui::Image<'static> {
    let uri = format!("bytes://{tex}.svg");
    let svg = tex_to_svg(tex);
    egui::Image::from_bytes(uri, svg).fit_to_original_size(1.5)
}

fn render_inference(mm: &lib::Context, stmt_addr: lib::StatementAddress) -> egui::Image<'static> {
    let stmt_name = mm.label(stmt_addr);
    let (hyps, conclusion) = mm.render_inference(stmt_addr);
    let hyps_tex = hyps.into_iter().fold(String::new(), |mut out, hyp| {
        write!(out, "\\displaystyle {hyp} \\\\ ").unwrap();
        out
    });
    let tex = format!(
        "
        \\boxed{{
            \\texttt{{ {stmt_name} }}
            \\frac{{
                \\color{{gray}}
                \\substack{{
                    {hyps_tex}
                }}
            }}{{
                {conclusion}
            }}
        }}
        "
    );
    tex_to_image(tex)
}

fn main() -> eframe::Result {
    let title = "Proof stack game";
    let mm = lib::Context::load("iset.mm", include_bytes!("/tmp/dump/iset.mm"));
    let mut state = mm.initial_state(None);

    eframe::run_simple_native(title, eframe::NativeOptions::default(), move |ctx, _| {
        egui_extras::install_image_loaders(ctx);
        ctx.set_theme(egui::Theme::Light);
        egui::CentralPanel::default().show(ctx, |ui| {
            let level_addr = state.current_level_stmt_addr;
            let level_name = mm.label(level_addr);
            let next_level = state.next_level(&mm);
            let level_goal = render_inference(&mm, level_addr);

            ui.heading(title);
            ui.heading(format!("Level {level_name}"));

            ui.add(level_goal);

            ui.columns(2, |uis| {
                // Deps
                uis[0].vertical(|ui| {
                    egui::ScrollArea::both()
                        .id_salt(0)
                        .drag_to_scroll(false)
                        .show(ui, |ui| match next_level {
                            Some(next_state) => {
                                if ui.button("Next level").clicked() {
                                    state = next_state;
                                }
                            }
                            None => {
                                for (stmt_addr, opt_next_state) in state.buttons(&mm) {
                                    let image = render_inference(&mm, stmt_addr);
                                    match opt_next_state {
                                        Some(next_state) => {
                                            if ui
                                                .add(egui::ImageButton::new(
                                                    image.bg_fill(egui::Color32::LIGHT_GRAY),
                                                ))
                                                .clicked()
                                            {
                                                state = next_state;
                                            }
                                        }
                                        None => {
                                            ui.add(egui::ImageButton::new(
                                                image.bg_fill(egui::Color32::GRAY),
                                            ));
                                        }
                                    }
                                }
                            }
                        });
                });

                // Stack
                uis[1].vertical(|ui| {
                    egui::ScrollArea::both()
                        .id_salt(1)
                        .drag_to_scroll(false)
                        .show(ui, |ui| {
                            for (i, expr) in state.render_stack(&mm).into_iter().enumerate() {
                                let image = tex_to_image(expr.clone());
                                ui.horizontal(|ui| {
                                    let id = egui::Id::new(&expr);
                                    let resp =
                                        ui.dnd_drag_source(id, i, |ui| ui.add(image)).response;
                                    if let Some(j) = resp.dnd_release_payload() {
                                        state = state.stack_move(*j, i);
                                    }
                                });
                            }
                        });
                });
            });
        });
    })
}
