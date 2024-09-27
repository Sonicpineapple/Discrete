use std::sync::Arc;

use cga2d::prelude::*;
use config::Settings;
use conformal_puzzle::{ConformalPuzzle, PuzzleDefinition, PuzzleEditor};
use eframe::{
    egui::{self, pos2, vec2, CollapsingHeader, Color32, Frame, Pos2, RichText, Shadow, Slider},
    epaint::PathShape,
};
use gfx::GfxData;
use group::{Generator, Point, Word};
mod conformal_puzzle;
use puzzle::GripSignature;
use regex::Regex;
use tiling::{QuotientGroup, Tiling};

mod config;
mod geom;
mod gfx;
mod group;
mod puzzle;
mod tiling;
mod todd_coxeter;

/// Native main function
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init();

    let native_options = eframe::NativeOptions {
        follow_system_theme: false,
        ..Default::default()
    };

    eframe::run_native(
        "Discrete",
        native_options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

/// Web main function
#[cfg(target_arch = "wasm32")]
fn main() {
    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions {
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            supported_backends: wgpu::Backends::GL, // TODO: can we allow WebGPU as well?
            ..eframe::egui_wgpu::WgpuConfiguration::default()
        },
        ..eframe::WebOptions::default()
    };

    wasm_bindgen_futures::spawn_local(async {
        let start_result = eframe::WebRunner::new()
            .start(
                "eframe_canvas",
                web_options,
                Box::new(|cc| Ok(Box::new(App::new(cc)))),
            )
            .await;

        // Remove the loading text and spinner:
        let loading_text = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("loading_text"));
        if let Some(loading_text) = loading_text {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}

enum Status {
    Invalid,
    Generated,
    Failed,
    Idle,
}
impl Status {
    fn message(&self) -> String {
        match self {
            Status::Invalid => "Invalid".to_string(),
            Status::Generated => "Generated".to_string(),
            Status::Failed => "Failed".to_string(),
            Status::Idle => "".to_string(),
        }
    }
}

struct Needs {
    puzzle_regenerate: bool,
    tiling_regenerate: bool,
}
impl Needs {
    fn new() -> Self {
        Self {
            puzzle_regenerate: false,
            tiling_regenerate: false,
        }
    }
}

struct App {
    settings: Settings,
    tiling: Arc<Tiling>,
    quotient_group: Arc<QuotientGroup>,
    gfx_data: GfxData,
    camera_transform: cga2d::Rotoflector,
    // puzzle_info: PuzzleInfo,
    // puzzle: Puzzle,
    puzzle_editor: Option<PuzzleEditor>,
    puzzle: Option<ConformalPuzzle>,
    needs: Needs,
    status: Status,
}
impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut gfx_data = GfxData::new(cc);

        let settings = Settings::new();
        let camera_transform = cga2d::Rotoflector::ident();

        let tiling = Arc::new(settings.tiling_settings.generate().unwrap());
        let quotient_group = Arc::new(tiling.get_quotient_group(settings.tile_limit).unwrap());
        // let puzzle_info = tiling.get_puzzle_info(settings.tile_limit).unwrap();
        // let puzzle = Puzzle::new_anticore_only(
        //     puzzle_info.element_group.clone(),
        //     puzzle_info.coset_group.clone(),
        // );
        let puzzle_def = PuzzleDefinition::new(tiling.clone(), quotient_group.clone());
        let puzzle = puzzle_def.generate_puzzle().unwrap();
        let needs = Needs::new();
        gfx_data.regenerate_puzzle_buffers(camera_transform, &puzzle);

        Self {
            settings,
            tiling,
            quotient_group,
            gfx_data,
            camera_transform,
            // puzzle_info,
            puzzle_editor: Some(PuzzleEditor::new(puzzle_def)),
            puzzle: Some(puzzle),
            needs,
            status: Status::Idle,
        }
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(Frame::none())
            .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();
                let (cen, size) = (rect.center(), rect.size());
                let unit = size.min_elem() / 2.;
                let boundary_circle = cga2d::circle(cga2d::NO, (size.max_elem() / unit) as f64);

                // Allocate space in the UI.
                let (egui_rect, target_size) =
                    rounded_pixel_rect(ui, ui.available_rect_before_wrap(), 1);

                let image = egui::widgets::Image::from_texture((
                    self.gfx_data.texture_id,
                    vec2(100., 100.),
                ));

                // Settings menu
                ui.with_layer_id(
                    egui::LayerId::new(egui::Order::Foreground, egui::Id::new("Settings")),
                    |ui| {
                        Frame::popup(ui.style())
                            .outer_margin(10.)
                            .shadow(Shadow::NONE)
                            // .stroke(Stroke::NONE)
                            .show(ui, |ui| {
                                CollapsingHeader::new("Settings").show(ui, |ui| {
                                    ui.collapsing("Tiling Settings", |ui| {
                                        ui.horizontal(|ui| {
                                            self.needs.tiling_regenerate |= ui
                                                .text_edit_singleline(
                                                    &mut self.settings.tiling_settings.schlafli,
                                                )
                                                .changed();
                                            ui.label(
                                                RichText::new("■").color(
                                                    match Regex::new(config::SCHLAFLI_PATTERN)
                                                        .unwrap()
                                                        .is_match(
                                                            &self.settings.tiling_settings.schlafli,
                                                        ) {
                                                        true => egui::Color32::GREEN,
                                                        false => egui::Color32::RED,
                                                    },
                                                ),
                                            );
                                        });
                                        ui.horizontal(|ui| {
                                            if ui.button("+").clicked() {
                                                self.settings
                                                    .tiling_settings
                                                    .relations
                                                    .push("".to_string());
                                                self.needs.tiling_regenerate = true;
                                            }
                                            if ui.button("-").clicked() {
                                                self.settings.tiling_settings.relations.pop();
                                                self.needs.tiling_regenerate = true;
                                            }
                                        });
                                        for rel in &mut self.settings.tiling_settings.relations {
                                            self.needs.tiling_regenerate |=
                                                ui.text_edit_singleline(rel).changed();
                                        }
                                        self.needs.tiling_regenerate |= ui
                                            .text_edit_singleline(
                                                &mut self.settings.tiling_settings.subgroup,
                                            )
                                            .changed();
                                    });
                                    ui.collapsing("View Settings", |ui| {
                                        ui.horizontal(|ui| {
                                            ui.add(
                                                Slider::new(
                                                    &mut self.settings.view_settings.col_scale,
                                                    0.1..=2.0,
                                                )
                                                .logarithmic(true),
                                            );
                                            ui.label("Colour Scale");
                                        });
                                        ui.horizontal(|ui| {
                                            ui.add(Slider::new(
                                                &mut self.settings.view_settings.outline_thickness,
                                                0.0..=1.0,
                                            ));
                                            ui.label("Outline Thickness")
                                        });
                                        ui.checkbox(
                                            &mut self.settings.view_settings.fundamental,
                                            "Draw fundamental region",
                                        );
                                        ui.checkbox(
                                            &mut self.settings.view_settings.mirrors,
                                            "Draw mirrors",
                                        );
                                        ui.checkbox(
                                            &mut self.settings.view_settings.path_debug,
                                            "Draw path",
                                        );
                                        ui.checkbox(
                                            &mut self.settings.view_settings.col_tiles,
                                            "Colour by quotient",
                                        );
                                        ui.checkbox(
                                            &mut self.settings.view_settings.inverse_col,
                                            "Colour by neighbours",
                                        );
                                    });
                                    if let Some(puzzle_editor) = &mut self.puzzle_editor {
                                        ui.collapsing("Puzzle Definition Editor", |ui| {
                                            for i in 0..puzzle_editor.puzzle_def.piece_types.len() {
                                                if ui.button(format!("Piece type {}", i)).clicked()
                                                {
                                                    puzzle_editor.active_piece_type = Some(i);
                                                }
                                            }
                                            if let Some(piece_type) =
                                                puzzle_editor.active_piece_type
                                            {
                                                ui.label(format!("Editing type {}", piece_type));
                                            }
                                            if ui.button("+").clicked() {
                                                puzzle_editor
                                                    .puzzle_def
                                                    .piece_types
                                                    .push(GripSignature::CORE);
                                            }
                                            if ui.button("Generate Puzzle").clicked() {
                                                puzzle_editor.active_piece_type = None;
                                                self.needs.puzzle_regenerate = true;
                                                // self.gfx_data.regenerate_cut_buffer(
                                                //     self.camera_transform,
                                                //     &puzzle,
                                                // );
                                                // self.gfx_data.regenerate_sticker_buffer(&puzzle);
                                            }
                                        });
                                    }
                                    // if let Some(puzzle) = &mut self.puzzle {
                                    //     ui.collapsing("Puzzle Settings", |ui| {
                                    //         if puzzle.editor.is_none() {
                                    //             if ui.button("Edit").clicked() {
                                    //                 puzzle.set_editor(0);
                                    //             }
                                    //         } else {
                                    //             for i in 0..puzzle.puzzle.piece_types.len() {
                                    //                 if ui
                                    //                     .button(format!("Piece type {}", i))
                                    //                     .clicked()
                                    //                 {
                                    //                     puzzle.set_editor(i);
                                    //                 }
                                    //             }
                                    //             if ui.button("Confirm").clicked() {
                                    //                 puzzle.apply_editor();
                                    //                 self.gfx_data.regenerate_cut_buffer(
                                    //                     self.camera_transform,
                                    //                     &puzzle,
                                    //                 );
                                    //                 self.gfx_data
                                    //                     .regenerate_sticker_buffer(&puzzle);
                                    //             }
                                    //         }
                                    //     });
                                    // }

                                    ui.horizontal(|ui| {
                                        ui.add(
                                            Slider::new(&mut self.settings.depth, 1..=100)
                                                .logarithmic(true),
                                        );
                                        ui.label("Iteration Depth");
                                    });
                                    ui.horizontal(|ui| {
                                        if ui
                                            .add(
                                                Slider::new(
                                                    &mut self.settings.tile_limit,
                                                    100..=5000,
                                                )
                                                .logarithmic(true),
                                            )
                                            .changed()
                                        {
                                            self.needs.tiling_regenerate = true;
                                        };
                                        ui.label("Tile Limit");
                                    });

                                    ui.horizontal(|ui| {
                                        if ui.button("Reset Camera").clicked() {
                                            self.camera_transform = cga2d::Rotoflector::ident();
                                        }
                                        self.needs.tiling_regenerate |=
                                            ui.button("Regenerate").clicked();
                                    });
                                    ui.label(self.status.message());
                                    if let Some(puzzle) = &self.puzzle {
                                        ui.label(
                                            puzzle.puzzle.grip_group.point_count().to_string(),
                                        );
                                    }
                                    // if ui.button("Move").clicked() {
                                    //     if self.puzzle.apply_move(Word(vec![]), 0, false).is_err() {
                                    //         self.status = Status::Invalid
                                    //     } else {
                                    //         self.gfx_data.regenerate_sticker_buffer(&self.puzzle);
                                    //         self.status = Status::Idle
                                    //     };
                                    // }
                                    // for mirror in self
                                    //     .tiling
                                    //     .mirrors
                                    //     .iter()
                                    //     .map(|&m| self.camera_transform.sandwich(m))
                                    // {
                                    //     let new_b = mirror.sandwich(cga2d::circle(NO, 1.));
                                    //     let badness = match new_b.unpack(0.0) {
                                    //         cga2d::LineOrCircle::Line { .. } => f32::INFINITY,
                                    //         cga2d::LineOrCircle::Circle { cx, cy, r } => {
                                    //             ((cx * cx + cy * cy) + r.ln() * r.ln()) as f32
                                    //         }
                                    //     };
                                    //     ui.label(badness.to_string());
                                    // }
                                    // let clicked = ui.button("Go Forth and Boop").clicked();
                                    // let mut trans = cga2d::Rotoflector::ident();
                                    // let mut mirrored = false;
                                    // let mut mirrors: Vec<Blade3> = self
                                    //     .tiling
                                    //     .mirrors
                                    //     .iter()
                                    //     .map(|&m| self.camera_transform.sandwich(m))
                                    //     .collect();
                                    // for _ in 0..10 {
                                    //     let mut badness: f32 = mirrors
                                    //         .iter()
                                    //         .map(|&m| {
                                    //             let new_b = m.sandwich(cga2d::circle(NO, 1.));
                                    //             match new_b.unpack(0.0) {
                                    //                 cga2d::LineOrCircle::Line { .. } => {
                                    //                     f32::INFINITY
                                    //                 }
                                    //                 cga2d::LineOrCircle::Circle { cx, cy, r } => {
                                    //                     ((cx * cx + cy * cy) + r.ln() * r.ln())
                                    //                         as f32
                                    //                 }
                                    //             }
                                    //         })
                                    //         .sum();
                                    //     let mut m_index = None;
                                    //     for (i, mirror) in mirrors.iter().enumerate() {
                                    //         let new_badness = mirrors
                                    //             .iter()
                                    //             .map(|&m| {
                                    //                 let m = mirror.sandwich(m);
                                    //                 let new_b = m.sandwich(cga2d::circle(NO, 1.));
                                    //                 match new_b.unpack(0.0) {
                                    //                     cga2d::LineOrCircle::Line { .. } => {
                                    //                         f32::INFINITY
                                    //                     }
                                    //                     cga2d::LineOrCircle::Circle {
                                    //                         cx,
                                    //                         cy,
                                    //                         r,
                                    //                     } => {
                                    //                         ((cx * cx + cy * cy) + r.ln() * r.ln())
                                    //                             as f32
                                    //                     }
                                    //                 }
                                    //             })
                                    //             .sum();
                                    //         if new_badness < badness {
                                    //             badness = new_badness;
                                    //             m_index = Some(i);
                                    //         }
                                    //     }
                                    //     if let Some(m_index) = m_index {
                                    //         ui.label(m_index.to_string());
                                    //         let mirror = mirrors[m_index];
                                    //         mirrors
                                    //             .iter_mut()
                                    //             .for_each(|m| *m = mirror.sandwich(*m));
                                    //         mirrored = !mirrored;
                                    //         trans = trans * mirror;
                                    //     }
                                    // }
                                    // if mirrored {
                                    //     trans = self.tiling.mirrors[0] * trans;
                                    // }
                                    // if true | clicked {
                                    //     self.camera_transform =
                                    //         (trans * self.camera_transform).normalize();
                                    // }
                                    // // let goodness = self
                                    // //     .tiling
                                    // //     .mirrors
                                    // //     .iter()
                                    // //     .map(|&m| {
                                    // //         let m = self.camera_transform.sandwich(m);
                                    // //         let new_b = m.sandwich(boundary_circle);
                                    // //         match new_b.unpack(0.01) {
                                    // //             cga2d::LineOrCircle::Line { .. } => f32::INFINITY,
                                    // //             cga2d::LineOrCircle::Circle { cx, cy, r } => {
                                    // //                 ((cx * cx + cy * cy) + r.ln() * r.ln()) as f32
                                    // //             }
                                    // //         }
                                    // //     })
                                    // //     .sum();
                                })
                            });
                    },
                );

                let r = ui.interact(
                    egui_rect,
                    eframe::egui::Id::new("Drawing"),
                    egui::Sense::click_and_drag(),
                );

                let scale = egui_rect.size() / (1. * egui_rect.size().min_elem());
                let scale = [scale.x, scale.y];

                let screen_to_egui =
                    |pos: Pos| pos2(pos.x as f32, -pos.y as f32) * unit + cen.to_vec2();
                let egui_to_screen = |pos: Pos2| {
                    let pos = (pos - cen.to_vec2()) / unit;
                    Pos {
                        x: pos.x as f64,
                        y: -pos.y as f64,
                    }
                };

                // Scroll zooming
                if r.hovered() {
                    let scroll_delta = ctx.input(|i| i.smooth_scroll_delta.y / unit);
                    if scroll_delta.abs() > 0.001 {
                        let scale = (NO ^ NI)
                            .connect(cga2d::point(1. + scroll_delta as f64 / 2., 0.))
                            * (NO ^ NI).connect(cga2d::point(1., 0.));
                        self.camera_transform = scale * self.camera_transform;
                        // self.scale = (self.scale - scroll_delta).max(0.1);
                        // unit = size.min_elem() / (2. * self.scale);
                    }
                }
                // Camera movement
                if r.dragged_by(egui::PointerButton::Secondary) {
                    if r.drag_delta().length() > 0.1 {
                        if let Some(mpos) = r.interact_pointer_pos() {
                            let egui_to_geom = |pos: Pos2| {
                                let Pos { x, y } = egui_to_screen(pos);
                                cga2d::point(x, y)
                            };
                            let root_pos = egui_to_geom(mpos - r.drag_delta());
                            let end_pos = egui_to_geom(mpos);

                            let modifiers = ctx.input(|i| i.modifiers);

                            let ms: Vec<cga2d::Blade3> = self
                                .tiling
                                .mirrors
                                .iter()
                                .map(|&m| self.camera_transform.sandwich(m))
                                .collect();
                            let boundary = match (modifiers.command, modifiers.alt) {
                                (true, false) => {
                                    let third = if self.tiling.rank == 4 {
                                        !ms[3]
                                    } else {
                                        !(!ms[0] ^ !ms[1] ^ !ms[2])
                                    };
                                    !ms[1] ^ !ms[2] ^ third
                                }
                                (false, true) => {
                                    let third = if self.tiling.rank == 4 {
                                        !ms[3]
                                    } else {
                                        !(!ms[0] ^ !ms[1] ^ !ms[2])
                                    };
                                    !ms[0] ^ !ms[1] ^ third
                                }
                                (true, true) => !cga2d::NI,
                                _ => !ms[0] ^ !ms[1] ^ !ms[2],
                            }; // the boundary to fix when transforming space

                            let init_refl = !(root_pos ^ end_pos) ^ !boundary; // get root_pos to end_pos
                            let f = end_pos ^ !boundary;
                            let final_refl = !(!init_refl ^ f) ^ f; // restore orientation fixing the "straight line" from root_pos to end_pos

                            self.camera_transform =
                                (final_refl * init_refl * self.camera_transform).normalize();
                        }
                    }
                }

                let camera_transform = self.camera_transform;
                let egui_to_geom = |pos: Pos2| {
                    let Pos { x, y } = egui_to_screen(pos);
                    camera_transform.rev().sandwich(cga2d::point(x, y))
                };
                let geom_to_egui = |pos: cga2d::Blade1| {
                    let (x, y) = camera_transform.sandwich(pos).unpack_point();
                    screen_to_egui(Pos { x, y })
                };
                // Move fundamental region to avoid noise
                if r.middle_clicked() {
                    if let Some(mpos) = ctx.pointer_latest_pos() {
                        let mut seed = egui_to_geom(mpos);
                        let mut word = Word(vec![]);
                        let mut trans = cga2d::Rotoflector::ident();
                        let mut mirrored = false;
                        for _ in 0..self.settings.depth {
                            let mut done = true;
                            for (i, &mirror) in self.tiling.mirrors.iter().enumerate() {
                                if !(mirror ^ seed) < 0. {
                                    let new_seed = mirror.sandwich(seed);
                                    seed = new_seed;
                                    done = false;
                                    word = word * Generator(i as u8);
                                    trans = trans * mirror;
                                    mirrored = !mirrored;
                                }
                            }
                            if done {
                                break;
                            }
                        }
                        if !mirrored {
                            self.camera_transform = (self.camera_transform * trans).normalize();
                        }
                    }
                }

                if self.needs.tiling_regenerate {
                    if let Ok(x) = self.settings.tiling_settings.generate() {
                        self.tiling = Arc::new(x);
                        if let Ok(q) = self.tiling.get_quotient_group(self.settings.tile_limit) {
                            self.quotient_group = Arc::new(q);
                            self.puzzle_editor = Some(PuzzleEditor::new(PuzzleDefinition::new(
                                self.tiling.clone(),
                                self.quotient_group.clone(),
                            )));
                            self.needs.puzzle_regenerate = true;
                        } else {
                            self.status = Status::Failed;
                        }
                    } else {
                        self.status = Status::Invalid;
                    }
                    self.needs.tiling_regenerate = false;
                }
                if self.needs.puzzle_regenerate {
                    if let Some(puzzle_editor) = &self.puzzle_editor {
                        if let Ok(puzzle) = puzzle_editor.puzzle_def.generate_puzzle() {
                            self.puzzle = Some(puzzle);
                            self.status = Status::Generated;
                            self.gfx_data.regenerate_puzzle_buffers(
                                self.camera_transform,
                                self.puzzle.as_ref().unwrap(),
                            );
                        } else {
                            self.status = Status::Failed;
                        };
                    }
                    self.needs.puzzle_regenerate = false;
                }
                if let Some(puzzle) = &self.puzzle {
                    self.gfx_data
                        .regenerate_cut_buffer(self.camera_transform, puzzle);
                }
                let mut outlines = vec![];
                let mirrors = &self.tiling.mirrors;
                let b_cell = !mirrors[0] ^ !mirrors[1] ^ !mirrors[2];
                if b_cell.mag2() > 0. {
                    let bp = b_cell & mirrors[2];
                    outlines.push(cga2d::slerp(
                        -mirrors[2],
                        bp ^ (b_cell.mag2().signum() * mirrors[0] & mirrors[1])
                            .unpack_point_pair()
                            .unwrap()[0],
                        std::f64::consts::PI / 2.
                            * self.settings.view_settings.outline_thickness as f64,
                    ));
                }
                let b_vert = !mirrors[1] ^ !mirrors[2] ^ !mirrors[3];
                if b_vert.mag2() > 0. {
                    let bp = b_vert & mirrors[3];
                    outlines.push(-cga2d::slerp(
                        mirrors[3],
                        bp ^ (b_vert.mag2().signum() * mirrors[1] & mirrors[2])
                            .unpack_point_pair()
                            .unwrap()[1],
                        std::f64::consts::PI / 2.
                            * self.settings.view_settings.outline_thickness as f64,
                    ));
                }
                self.gfx_data
                    .regenerate_outline_buffer(camera_transform, &outlines);
                self.gfx_data.frame(
                    gfx::Params::new(
                        self.tiling
                            .mirrors
                            .iter()
                            .map(|&m| self.camera_transform.sandwich(m))
                            .collect(),
                        self.tiling.edges.clone(),
                        if let Some(mpos) = ctx.pointer_latest_pos() {
                            egui_to_geom(mpos)
                        } else {
                            cga2d::point(0., 1.)
                        },
                        scale,
                        if let Some(puzzle) = &self.puzzle {
                            puzzle.cut_circles.len()
                        } else {
                            0
                        },
                        outlines.len(),
                        self.settings.depth,
                        &self.settings.view_settings,
                    ),
                    target_size[0],
                    target_size[1],
                );
                ui.with_layer_id(egui::LayerId::background(), |ui| {
                    image.paint_at(ui, egui_rect);
                });
                // ui.put(egui_rect, image);

                // debug dots
                // ui.painter().circle_filled(
                //     screen_to_egui(Pos { x: 1., y: 0. }),
                //     5.,
                //     egui::Color32::GOLD,
                // );
                // ui.painter().circle_filled(
                //     screen_to_egui(Pos { x: 0., y: 1. }),
                //     5.,
                //     egui::Color32::GOLD,
                // );

                let cols = [
                    egui::Color32::RED,
                    egui::Color32::GREEN,
                    egui::Color32::BLUE,
                    egui::Color32::YELLOW,
                    egui::Color32::KHAKI,
                    egui::Color32::BLACK,
                ];
                let stroke_width = 1.;

                let draw_circle = |mirror: cga2d::Blade3, col_index, stroke_width: f32| {
                    // Find the point pair where the mirror intersects the visible region.
                    let pp = mirror & boundary_circle;
                    if let Some(_) = pp.unpack_point_pair() {
                        let mid = pp.sandwich(cga2d::NI);
                        let perpendicular_pp = pp.connect(mid) & mirror;

                        // Sample points uniformly along the mirror.
                        const CURVE_SAMPLE_COUNT: usize = 200;
                        let points = (0..=CURVE_SAMPLE_COUNT)
                            .filter_map(|i| {
                                // Interpolate along a straight line.
                                let t = i as f64 / CURVE_SAMPLE_COUNT as f64;
                                let [sample_point, _] =
                                    cga2d::slerp(pp, perpendicular_pp, t * std::f64::consts::PI)
                                        .unpack_point_pair()?;
                                Some(sample_point.unpack_point())
                            })
                            .map(|(x, y)| screen_to_egui(Pos { x, y }))
                            .collect();
                        ui.painter().add(PathShape {
                            points,
                            closed: false,
                            fill: Color32::TRANSPARENT,
                            stroke: (stroke_width, cols[col_index]).into(),
                        });
                    } else {
                        match mirror.unpack(0.001) {
                            cga2d::LineOrCircle::Line { .. } => (), // does not intersect view
                            cga2d::LineOrCircle::Circle { cx, cy, r } => {
                                ui.painter().circle_stroke(
                                    screen_to_egui(Pos::new(cx, cy)),
                                    (r * unit as f64) as _,
                                    (stroke_width, cols[col_index]),
                                );
                            }
                        }
                    }
                };
                if self.settings.view_settings.mirrors {
                    for (i, mirror) in self
                        .tiling
                        .mirrors
                        .iter()
                        .map(|&m| self.camera_transform.sandwich(m))
                        .enumerate()
                    {
                        draw_circle(mirror, i, stroke_width);
                    }
                }
                if let Some(puzzle_editor) = &self.puzzle_editor {
                    if let Some(active_piece_type) = puzzle_editor.active_piece_type {
                        let stroke_width = 3.;
                        let circ = if self.tiling.rank == 3 {
                            !self.tiling.mirrors[0]
                                ^ !self.tiling.mirrors[1]
                                ^ cga2d::point(0.3, 0.)
                        } else {
                            !self.tiling.mirrors[0]
                                ^ !self.tiling.mirrors[1]
                                ^ !self.tiling.mirrors[2]
                        };
                        for grip in &puzzle_editor.puzzle_def.piece_types[active_piece_type].0 {
                            let word = &self.quotient_group.tile_group.word_table[grip.0 as usize];
                            draw_circle(
                                self.camera_transform
                                    .sandwich(word.0.iter().fold(circ, |c, g| {
                                        self.tiling.mirrors[g.0 as usize].sandwich(c)
                                    })),
                                5,
                                stroke_width,
                            );
                        }
                        for cut in &puzzle_editor.puzzle_def.cut_circles {
                            draw_circle(self.camera_transform.sandwich(*cut), 4, stroke_width);
                        }
                    }
                };

                if r.is_pointer_button_down_on() {
                    if let Some(mpos) = ctx.pointer_latest_pos() {
                        let mut seed = egui_to_geom(mpos);

                        // Fill regions
                        if ui.input(|i| i.pointer.primary_down()) {
                            ui.painter()
                                .circle_filled(geom_to_egui(seed), 5., egui::Color32::GRAY);
                            // for (i, &mirror) in self.tiling.mirrors.iter().enumerate() {
                            //     if !(mirror ^ seed) < 0. {
                            //         ui.painter().circle_filled(
                            //             geom_to_egui(mirror.sandwich(seed)),
                            //             5.,
                            //             cols[i],
                            //         );
                            //     }
                            // }

                            let mut word = Word(vec![]);
                            let circ = !self.tiling.mirrors[0]
                                ^ !self.tiling.mirrors[1]
                                ^ !self.tiling.mirrors[2];
                            let mut mirrored = false;
                            for _ in 0..self.settings.depth {
                                let mut done = true;
                                for (i, &mirror) in self.tiling.mirrors.iter().enumerate() {
                                    if !(mirror ^ seed) < 0. {
                                        let new_seed = mirror.sandwich(seed);
                                        if self.settings.view_settings.path_debug {
                                            ui.painter().line_segment(
                                                [geom_to_egui(seed), geom_to_egui(new_seed)],
                                                (3., cols[i]),
                                            );
                                            ui.painter().circle_filled(
                                                geom_to_egui(new_seed),
                                                5.,
                                                egui::Color32::LIGHT_GRAY,
                                            );
                                        }
                                        seed = new_seed;
                                        done = false;
                                        word = word * Generator(i as u8);
                                        mirrored = !mirrored;
                                    }
                                }
                                if done {
                                    break;
                                }
                            }
                            draw_circle(
                                self.camera_transform.sandwich(
                                    word.inverse().0.iter().fold(circ, |c, g| {
                                        self.tiling.mirrors[g.0 as usize].sandwich(c)
                                    }),
                                ),
                                4,
                                stroke_width,
                            );
                            if ctx.input(|i| i.pointer.primary_pressed()) {
                                if let Some(puzzle_editor) = &mut self.puzzle_editor {
                                    if let Some(active_piece_type) = puzzle_editor.active_piece_type
                                    {
                                        if word.0.len() == 0 {
                                            let mask = puzzle_editor.puzzle_def.get_cut_mask(seed);
                                            if puzzle_editor.puzzle_def.cut_map[mask]
                                                == Some(active_piece_type)
                                            {
                                                puzzle_editor.puzzle_def.cut_map[mask] = None;
                                            } else {
                                                puzzle_editor.puzzle_def.cut_map[mask] =
                                                    Some(active_piece_type);
                                            }
                                        } else {
                                            if let Some(grip) = self
                                                .quotient_group
                                                .tile_group
                                                .mul_word(&Point::INIT, &word.inverse())
                                            {
                                                // TODO: hide this
                                                if puzzle_editor.puzzle_def.piece_types
                                                    [active_piece_type]
                                                    .contains(&grip)
                                                {
                                                    puzzle_editor.puzzle_def.piece_types
                                                        [active_piece_type]
                                                        .0
                                                        .retain(|g| g.0 != grip.0);
                                                } else {
                                                    puzzle_editor.puzzle_def.piece_types
                                                        [active_piece_type]
                                                        .0
                                                        .push(grip);
                                                }
                                            }
                                        }
                                    } else {
                                        if let Some(puzzle) = &mut self.puzzle {
                                            if puzzle.apply_move(word, 0, false).is_err() {
                                                self.status = Status::Invalid
                                            } else {
                                                self.gfx_data.regenerate_sticker_buffer(&puzzle);
                                                self.status = Status::Idle
                                            };
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            });
    }
}

#[derive(Debug, Default, Copy, Clone)]
struct Pos {
    x: f64,
    y: f64,
}
impl Pos {
    fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}
impl From<Pos> for Pos2 {
    fn from(value: Pos) -> Self {
        Self {
            x: value.x as f32,
            y: value.y as f32,
        }
    }
}

/// Rounds an egui rectangle to the nearest pixel boundary and returns the
/// rounded egui rectangle, along with its width & height in pixels.
pub fn rounded_pixel_rect(
    ui: &egui::Ui,
    rect: egui::Rect,
    downscale_rate: u32,
) -> (egui::Rect, [u32; 2]) {
    let dpi = ui.ctx().pixels_per_point();

    // Round rectangle to pixel boundary for crisp image.
    let mut pixels_rect = rect;
    pixels_rect.set_left((dpi * pixels_rect.left()).ceil());
    pixels_rect.set_bottom((dpi * pixels_rect.bottom()).floor());
    pixels_rect.set_right((dpi * pixels_rect.right()).floor());
    pixels_rect.set_top((dpi * pixels_rect.top()).ceil());

    // Convert back from pixel coordinates to egui coordinates.
    let mut egui_rect = pixels_rect;
    *egui_rect.left_mut() /= dpi;
    *egui_rect.bottom_mut() /= dpi;
    *egui_rect.right_mut() /= dpi;
    *egui_rect.top_mut() /= dpi;

    let pixel_size = [
        pixels_rect.width() as u32 / downscale_rate,
        pixels_rect.height() as u32 / downscale_rate,
    ];
    (egui_rect, pixel_size)
}
