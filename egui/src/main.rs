use eframe::egui;
use memoize::memoize;
use std::fmt::Write;

#[memoize]
fn tex_to_svg(tex: String) -> Vec<u8> {
    mathjax_svg::convert_to_svg(tex).unwrap().into_bytes()
}

// egui supports svg, but scaling is blurry for some reason,
// so manually scale and rasterize to png
fn tex_to_image(ppp: f32, tex: String) -> egui::Image<'static> {
    let uri = format!("bytes://{tex}.png");
    let svg = tex_to_svg(tex);
    let svg = resvg::usvg::Tree::from_data(&svg, &Default::default()).unwrap();
    let scale = 2.0 * ppp;
    let size = svg.size().to_int_size().scale_by(scale).unwrap();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height()).unwrap();
    resvg::render(
        &svg,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let png = pixmap.encode_png().unwrap();
    egui::Image::from_bytes(uri, png).fit_to_original_size(1.0 / ppp)
}

fn render_inference(
    ppp: f32,
    mm: &lib::Context,
    stmt_addr: lib::StatementAddress,
) -> egui::Image<'static> {
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
                \\substack{{
                    {hyps_tex}
                }}
            }}{{
                {conclusion}
            }}
        }}
        "
    );
    tex_to_image(ppp, tex)
}

fn main() -> eframe::Result {
    let title = "Proof stack game";
    let mm = lib::Context::load(
        "set.mm",
        include_bytes!("/tmp/dump/set.mm"),
        lib::TypesetMode::Latex,
    );
    let mut state = mm.initial_state(Some("dftru2"));

    eframe::run_simple_native(title, eframe::NativeOptions::default(), move |ctx, _| {
        egui_extras::install_image_loaders(ctx);
        ctx.set_theme(egui::Theme::Light);
        egui::CentralPanel::default().show(ctx, |ui| {
            let level_addr = state.current_level_stmt_addr;
            let level_name = mm.label(level_addr);
            let next_level = state.next_level(&mm);
            let level_goal = render_inference(ctx.pixels_per_point(), &mm, level_addr);

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
                                    let image =
                                        render_inference(ctx.pixels_per_point(), &mm, stmt_addr);
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
                                let image = tex_to_image(ctx.pixels_per_point(), expr.clone());
                                let id = egui::Id::new(i);
                                ui.horizontal(|ui| {
                                    if ui.button("Del").clicked() {
                                        state = state.stack_delete(i).unwrap();
                                    }
                                    if ui.button("Cpy").clicked() {
                                        state = state.stack_copy(i).unwrap();
                                    }
                                    if let Some(j) = ui
                                        .dnd_drag_source(id, i, |ui| ui.add(image))
                                        .response
                                        .dnd_release_payload()
                                    {
                                        state = state.stack_move(*j, i).unwrap();
                                    }
                                });
                            }
                        });
                });
            });
        });
    })
}
