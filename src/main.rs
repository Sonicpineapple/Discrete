use cga2d::{Blade, Multivector};
use config::{PuzzleInfo, Schlafli, Settings, Tiling, ViewSettings};
use eframe::{
    egui::{self, pos2, vec2, CollapsingHeader, Color32, Frame, Pos2, RichText, Shadow, Slider},
    epaint::PathShape,
};
use gfx::GfxData;
use group::{Generator, Point, Word};
use puzzle::Puzzle;
use regex::Regex;

mod config;
mod geom;
mod gfx;
mod group;
mod puzzle;
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

struct App {
    scale: f32,
    settings: Settings,
    tiling: Tiling,
    gfx_data: GfxData,
    camera_transform: cga2d::Rotor,
    puzzle_info: PuzzleInfo,
    puzzle: Puzzle,
    needs_regenerate: bool,
    status: Status,
}
impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let gfx_data = GfxData::new(cc);

        let settings = Settings::new();

        let tiling = settings.tiling_settings.generate().unwrap();

        let puzzle_info = tiling.get_puzzle_info(settings.tile_limit).unwrap();
        let puzzle = Puzzle::new_anticore_only(
            puzzle_info.element_group.clone(),
            puzzle_info.coset_group.clone(),
        );

        Self {
            scale: 1.,
            settings,
            tiling,
            gfx_data,
            camera_transform: cga2d::Rotor::ident(),
            puzzle_info,
            puzzle,
            needs_regenerate: true,
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
                let mut unit = size.min_elem() / (2. * self.scale);
                let boundary_circle = cga2d::circle(cga2d::NO, (size.max_elem() / unit) as f64);

                // Allocate space in the UI.
                let (egui_rect, target_size) =
                    rounded_pixel_rect(ui, ui.available_rect_before_wrap(), 1);

                let image = egui::widgets::Image::from_texture((
                    self.gfx_data.texture_id,
                    vec2(100., 100.),
                ));

                let r = ui.interact(
                    egui_rect,
                    eframe::egui::Id::new("Drawing"),
                    egui::Sense::click_and_drag(),
                );

                // Scroll zooming
                if r.hovered() {
                    let scroll_delta = ctx.input(|i| i.smooth_scroll_delta.y / unit);
                    if scroll_delta.abs() > 0.001 {
                        self.scale = (self.scale - scroll_delta).max(0.1);
                        unit = size.min_elem() / (2. * self.scale);
                    }
                }

                let scale = egui_rect.size() / (1. * egui_rect.size().min_elem());
                let scale = [scale.x * self.scale, scale.y * self.scale];

                let screen_to_egui =
                    |pos: Pos| pos2(pos.x as f32, -pos.y as f32) * unit + cen.to_vec2();
                let egui_to_screen = |pos: Pos2| {
                    let pos = (pos - cen.to_vec2()) / unit;
                    Pos {
                        x: pos.x as f64,
                        y: -pos.y as f64,
                    }
                };

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

                let egui_to_geom = |pos: Pos2| {
                    let Pos { x, y } = egui_to_screen(pos);
                    self.camera_transform.rev().sandwich(cga2d::point(x, y))
                };
                let geom_to_egui = |pos: cga2d::Blade1| {
                    let (x, y) = self.camera_transform.sandwich(pos).unpack_point();
                    screen_to_egui(Pos { x, y })
                };

                if self.needs_regenerate {
                    self.puzzle = Puzzle::new_anticore_only(
                        self.puzzle_info.element_group.clone(),
                        self.puzzle_info.coset_group.clone(),
                    );
                    self.gfx_data
                        .regenerate_puzzle_buffers(&self.puzzle_info, &self.puzzle);
                    self.needs_regenerate = false;
                }
                let cut_circle;
                {
                    let ms: Vec<cga2d::Blade3> = self.tiling.mirrors.iter().map(|&m| m).collect();
                    let p = (ms[0] & ms[1]);
                    cut_circle = cga2d::slerp(
                        ms[2],
                        -ms[2].connect(p).connect(p),
                        std::f64::consts::PI / 4.,
                    );
                    // let boundary = !ms[0] ^ !ms[1] ^ !ms[2];
                    // let p = ms[1].sandwich(!(ms[2] & ms[1]));
                    // cut_circle = ms[2].connect(p ^ !boundary).connect(p ^ !boundary);
                }
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
                        self.camera_transform.sandwich(cut_circle),
                        scale,
                        self.settings.depth,
                        &self.settings.view_settings,
                    ),
                    target_size[0],
                    target_size[1],
                );
                image.paint_at(ui, egui_rect);
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
                    egui::Color32::GOLD,
                ];
                let stroke_width = 1.;

                let draw_circle = |mirror: cga2d::Blade3, col_index| {
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
                        .chain([&cut_circle])
                        .map(|&m| self.camera_transform.sandwich(m))
                        .enumerate()
                    {
                        draw_circle(mirror, i);
                    }
                }
                if r.is_pointer_button_down_on() {
                    if let Some(mpos) = ctx.pointer_latest_pos() {
                        //let mpos = itrans(mpos);
                        let mut seed = egui_to_geom(mpos);
                        // let seed = Pos::new(seed.x as f64, -seed.y as f64);

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
                            );
                            if ctx.input(|i| i.pointer.primary_pressed()) {
                                if let Some(coset) = self
                                    .puzzle_info
                                    .coset_group
                                    .mul_word(&Point::INIT, &word.inverse())
                                {
                                    let init_turn = if mirrored {
                                        &Word(vec![Generator(1), Generator(0)])
                                    } else {
                                        &Word(vec![Generator(0), Generator(1)])
                                    };
                                    let turn = &word * init_turn * word.inverse();
                                    if self.puzzle.apply_move(&coset, &turn).is_err() {
                                        self.status = Status::Invalid
                                    } else {
                                        self.gfx_data.regenerate_sticker_buffer(
                                            &self.puzzle_info,
                                            &self.puzzle,
                                        );
                                        self.status = Status::Idle
                                    };
                                };
                            }
                        }
                    }
                }

                // Settings menu
                Frame::popup(ui.style())
                    .outer_margin(10.)
                    .shadow(Shadow::NONE)
                    // .stroke(Stroke::NONE)
                    .show(ui, |ui| {
                        CollapsingHeader::new("Settings").show(ui, |ui| {
                            let mut needs_try_regenerate = false;
                            ui.collapsing("Tiling Settings", |ui| {
                                ui.horizontal(|ui| {
                                    needs_try_regenerate |= ui
                                        .text_edit_singleline(
                                            &mut self.settings.tiling_settings.schlafli,
                                        )
                                        .changed();
                                    ui.label(
                                        RichText::new("■").color(
                                            match Regex::new(config::SCHLAFLI_PATTERN)
                                                .unwrap()
                                                .is_match(&self.settings.tiling_settings.schlafli)
                                            {
                                                true => egui::Color32::GREEN,
                                                false => egui::Color32::RED,
                                            },
                                        ),
                                    );
                                });
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 0.;
                                    for (i, val) in self.tiling.edges.iter_mut().enumerate() {
                                        if i < (self.tiling.rank as usize) {
                                            ui.checkbox(val, "");
                                            if i < (self.tiling.rank as usize - 1) {
                                                ui.label(
                                                    self.tiling.schlafli.0[i]
                                                        .map_or("i".to_string(), |x| x.to_string()),
                                                );
                                            }
                                        }
                                    }
                                });
                                for rel in &mut self.settings.tiling_settings.relations {
                                    needs_try_regenerate |= ui.text_edit_singleline(rel).changed();
                                }
                                ui.horizontal(|ui| {
                                    if ui.button("+").clicked() {
                                        self.settings
                                            .tiling_settings
                                            .relations
                                            .push("".to_string());
                                        needs_try_regenerate = true;
                                    }
                                    if ui.button("-").clicked() {
                                        self.settings.tiling_settings.relations.pop();
                                        needs_try_regenerate = true;
                                    }
                                });
                                needs_try_regenerate |= ui
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
                                        Slider::new(&mut self.settings.tile_limit, 100..=5000)
                                            .logarithmic(true),
                                    )
                                    .changed()
                                {
                                    needs_try_regenerate = true;
                                };
                                ui.label("Tile Limit");
                            });

                            ui.horizontal(|ui| {
                                if ui.button("Reset Camera").clicked() {
                                    self.camera_transform = cga2d::Rotor::ident();
                                }
                                needs_try_regenerate |= ui.button("Regenerate").clicked();
                            });
                            if needs_try_regenerate {
                                if let Ok(x) = self.settings.tiling_settings.generate() {
                                    if let Ok(info) = x.get_puzzle_info(self.settings.tile_limit) {
                                        self.puzzle_info = info;
                                        self.status = Status::Generated;
                                    } else {
                                        self.status = Status::Failed;
                                    }
                                    self.tiling = x;
                                    self.needs_regenerate = true;
                                    ctx.request_repaint();
                                } else {
                                    self.status = Status::Invalid;
                                }
                            }
                            ui.label(self.status.message());
                            ui.label(self.puzzle_info.coset_group.point_count().to_string());
                            if ui.button("Move").clicked() {
                                if self
                                    .puzzle
                                    .apply_move(&Point(0), &Word(vec![Generator(0), Generator(1)]))
                                    .is_err()
                                {
                                    self.status = Status::Invalid
                                } else {
                                    self.gfx_data
                                        .regenerate_sticker_buffer(&self.puzzle_info, &self.puzzle);
                                    self.status = Status::Idle
                                };
                            }
                        })
                    });
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
