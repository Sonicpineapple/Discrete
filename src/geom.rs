use core::f64;

use cga2d::prelude::*;

pub(crate) fn rank_3_mirrors(a: usize, b: usize) -> [Blade3; 3] {
    let a1 = f64::consts::PI / a as f64;
    let a2 = f64::consts::PI / b as f64;
    rank_3_mirrors_internal(a1, a2)
}

pub(crate) fn rank_4_mirrors(a: usize, b: usize, c: usize) -> [Blade3; 4] {
    let a1 = f64::consts::PI / a as f64;
    let a2 = f64::consts::PI / b as f64;
    let a3 = f64::consts::PI / c as f64;
    let [m1, m2, m3] = rank_3_mirrors_internal(a1, a2);
    let m4 = rank_4_last_mirror_internal(m1, m2, m3, a1, a2, a3);
    [m1, m2, m3, m4]
}

fn rank_3_mirrors_internal(a1: f64, a2: f64) -> [Blade3; 3] {
    let x_unit = cga2d::point(1., 0.);
    let mirror1 = NO ^ x_unit ^ NI;
    let mirror2 = (NO << mirror1).rotate(a1) ^ NI;

    // this is kind of magic? u is symmetric with the desired mirror3
    let q3 = (x_unit << mirror1) ^ !mirror2;
    let u = (mirror2 & q3).rotate(a2) ^ x_unit;
    // let vertex_2_3 = (u & mirror1).unpack_point_pair().expect("what")[1];
    let mirror3 = mirror1.connect(u & mirror1);
    // let mirror3 = dbg!(!mirror1) ^ dbg!(x_unit) ^ dbg!(vertex_2_3);
    [mirror1, mirror2, mirror3]
}

fn rank_4_last_mirror_internal(
    mirror1: Blade3,
    mirror2: Blade3,
    mirror3: Blade3,
    a1: f64,
    a2: f64,
    a3: f64,
) -> Blade3 {
    let mutual_perpendicular = !(mirror1 & mirror2 & mirror3);
    let temp_angle = (a3.sin() * a1.sin() / a2.cos()).asin();
    let temp_line = cga2d::slerp(mirror1, !mutual_perpendicular ^ !mirror1 ^ NO, -temp_angle);
    let vertex_3_4 = (temp_line & mirror3).unpack_point_pair().expect("what")[1];
    let mirror4 = !mirror1 ^ !mirror2 ^ vertex_3_4;
    mirror4
}
