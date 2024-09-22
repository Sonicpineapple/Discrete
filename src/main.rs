use cga2d::Multivector;
use eframe::egui::{self, pos2, vec2, Frame, Pos2};
use geom::{rank_3_mirrors, rank_4_mirrors};
use gfx::GfxData;
use todd_coxeter::Tables;

mod geom;
mod gfx;
mod group;
mod todd_coxeter;

fn main() -> eframe::Result<()> {
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

struct App {
    scale: f32,
    mirrors: [cga2d::Blade3; 4],
    gfx_data: GfxData,
    camera_transform: cga2d::Rotor,
}
impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let gfx_data = GfxData::new(cc);

        let gen_count = 4;

        // mirrors are assumed self-inverse
        // let mut rels = schlafli_rels(vec![7, 3]);
        // rels.push((0..8).flat_map(|_| [0, 2, 1]).collect());
        let mut rels = schlafli_rels(vec![8, 3, 3]);
        rels.push((0..2).flat_map(|_| [0, 2, 1, 0, 2, 1, 0, 1]).collect());

        let subgroup = vec![0, 1]; // generators are assumed mirrors

        let mut tables = Tables::new(gen_count, &rels, &subgroup);
        loop {
            if !tables.discover_next_unknown() {
                println!("Done");
                break;
            }
        }

        print!("{}", tables.coset_group());

        let mirrors = rank_4_mirrors(8, 3, 3);
        Self {
            scale: 1.,
            mirrors,
            gfx_data,
            camera_transform: cga2d::Rotor::ident(),
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

                // Allocate space in the UI.
                let (egui_rect, target_size) =
                    rounded_pixel_rect(ui, ui.available_rect_before_wrap(), 1);
                let r = ui.allocate_rect(egui_rect, egui::Sense::click_and_drag());

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

                if r.dragged_by(egui::PointerButton::Secondary) {
                    if r.drag_delta().length() > 0.1 {
                        let drag = r.drag_delta() / unit;
                        let drag = Pos::new(drag.x as f64, -drag.y as f64);

                        if let Some(mpos) = r.interact_pointer_pos() {
                            let egui_to_geom = |pos: Pos2| {
                                let Pos { x, y } = egui_to_screen(pos);
                                cga2d::point(x, y)
                            };
                            let root_pos = egui_to_geom(mpos - r.drag_delta());
                            let end_pos = egui_to_geom(mpos);

                            let boundary = !self.mirrors[0] ^ !self.mirrors[1] ^ !self.mirrors[2]; // the boundary to fix when transforming space

                            let init_refl = !(root_pos ^ end_pos) ^ !boundary; // get root_pos to end_pos
                            let f = end_pos ^ !boundary;
                            let final_refl = !(!init_refl ^ f) ^ f; // restore orientation fixing the "straight line" from root_pos to end_pos

                            self.camera_transform =
                                (final_refl * init_refl * self.camera_transform).normalize();
                            println!("{}", self.camera_transform);
                        }
                    }
                }

                let egui_to_geom = |pos: Pos2| {
                    let Pos { x, y } = egui_to_screen(pos);
                    self.camera_transform.rev().sandwich(cga2d::point(x, y))
                    // cga2d::point(x, y)
                };
                let geom_to_egui = |pos: cga2d::Blade1| {
                    let (x, y) = self.camera_transform.sandwich(pos).unpack_point();
                    // let (x, y) = pos.unpack_point();
                    screen_to_egui(Pos { x, y })
                };

                self.gfx_data.frame(
                    gfx::Params::new(
                        self.mirrors
                            .iter()
                            .map(|&m| self.camera_transform.sandwich(m))
                            .collect(),
                        if let Some(mpos) = ctx.pointer_latest_pos() {
                            egui_to_geom(mpos)
                        } else {
                            cga2d::point(0., 1.)
                        },
                        scale,
                        // self.camera_transform,
                    ),
                    target_size[0],
                    target_size[1],
                );
                let image = egui::widgets::Image::from_texture((
                    self.gfx_data.texture_id,
                    vec2(100., 100.),
                ));
                image.paint_at(ui, egui_rect);
                // ui.put(egui_rect, image);

                ui.painter().circle_filled(
                    screen_to_egui(Pos { x: 1., y: 0. }),
                    5.,
                    egui::Color32::GOLD,
                );
                ui.painter().circle_filled(
                    screen_to_egui(Pos { x: 0., y: 1. }),
                    5.,
                    egui::Color32::GOLD,
                );

                let cols = [
                    egui::Color32::RED,
                    egui::Color32::GREEN,
                    egui::Color32::BLUE,
                    egui::Color32::YELLOW,
                ];
                let stroke_width = 1.;
                for (i, mirror) in self
                    .mirrors
                    .iter()
                    .map(|&m| self.camera_transform.sandwich(m))
                    .enumerate()
                {
                    match mirror.unpack(0.0000001) {
                        cga2d::LineOrCircle::Line { .. } => {
                            let pp = mirror & cga2d::circle(cga2d::NO, 2. * unit as f64);
                            if pp.mag2() > 0. {
                                ui.painter().line_segment(
                                    if let Some(pp) = pp.unpack_point_pair() {
                                        pp.map(|p| {
                                            let (x, y) = p.unpack_point();
                                            screen_to_egui(Pos { x, y })
                                        })
                                    } else {
                                        todo!()
                                    },
                                    (stroke_width, cols[i]),
                                );
                            }
                        }
                        cga2d::LineOrCircle::Circle { cx, cy, r } => {
                            ui.painter().circle_stroke(
                                screen_to_egui(Pos::new(cx, cy)),
                                (r * unit as f64) as _,
                                (stroke_width, cols[i]),
                            );
                        }
                    };
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
                            for (i, &mirror) in self.mirrors.iter().enumerate() {
                                if !(mirror ^ seed) < 0. {
                                    ui.painter().circle_filled(
                                        geom_to_egui(mirror.sandwich(seed)),
                                        5.,
                                        cols[i],
                                    );
                                }
                            }
                            for _ in 0..100 {
                                let mut done = true;
                                for (i, &mirror) in self.mirrors.iter().enumerate() {
                                    if !(mirror ^ seed) < 0. {
                                        let new_seed = mirror.sandwich(seed);
                                        ui.painter().line_segment(
                                            [geom_to_egui(seed), geom_to_egui(new_seed)],
                                            (3., cols[i]),
                                        );
                                        ui.painter().circle_filled(
                                            geom_to_egui(new_seed),
                                            5.,
                                            egui::Color32::LIGHT_GRAY,
                                        );
                                        seed = new_seed;
                                        done = false;
                                    }
                                }
                                if done {
                                    break;
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

fn schlafli_rels(vals: Vec<u8>) -> Vec<Vec<u8>> {
    let mut rels = vec![];
    for (i, &val) in vals.iter().enumerate() {
        for x in 0..i {
            rels.push((0..2).flat_map(|_| [x as u8, i as u8 + 1]).collect());
        }
        rels.push((0..val).flat_map(|_| [i as u8, i as u8 + 1]).collect());
    }
    rels
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
