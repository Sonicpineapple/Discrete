use core::f64;

use cga2d::prelude::*;

fn angle(x: Option<usize>) -> f64 {
    f64::consts::PI / x.map_or(f64::INFINITY, |x| x as f64)
}

pub(crate) fn rank_3_mirrors(a: Option<usize>, b: Option<usize>) -> Result<[Blade3; 3], ()> {
    let a1 = angle(a);
    let a2 = angle(b);
    rank_3_mirrors_internal(a1, a2)
}

pub(crate) fn rank_4_mirrors(
    a: Option<usize>,
    b: Option<usize>,
    c: Option<usize>,
) -> Result<[Blade3; 4], ()> {
    let a1 = angle(a);
    let a2 = angle(b);
    let a3 = angle(c);
    let [m1, m2, m3] = rank_3_mirrors_internal(a1, a2)?;
    let m4 = rank_4_last_mirror_internal(m1, m2, m3, a1, a2, a3)?;
    // We generate the world "backwards", so invert everyone first
    let mut mirrors = [m1, m2, m3, m4];
    mirrors.iter_mut().for_each(|m| *m = -(m4).sandwich(*m));
    Ok(mirrors)
}

fn rank_3_mirrors_internal(a1: f64, a2: f64) -> Result<[Blade3; 3], ()> {
    let x_unit = cga2d::point(1., 0.);
    let mirror1 = NO ^ x_unit ^ NI;
    let mirror2 = if a1 > 0. {
        (NO << mirror1).rotate(a1) ^ NI
    } else {
        (NO << mirror1) ^ cga2d::point(0., 1.)
    };

    // this is kind of magic? u is symmetric with the desired mirror3
    let q3 = (x_unit << mirror1) ^ !mirror2;
    let u = (mirror2 & q3).rotate(a2) ^ x_unit;
    let vertex_2_3 = (u & mirror2).unpack_point_pair().ok_or(())?[0];
    let mirror3 = !mirror1 ^ x_unit ^ vertex_2_3;
    // let mirror3 = mirror1.connect(u & mirror2);
    Ok([
        mirror1.normalize(),
        mirror2.normalize(),
        mirror3.normalize(),
    ])
}

fn rank_4_last_mirror_internal(
    mirror1: Blade3,
    mirror2: Blade3,
    mirror3: Blade3,
    a1: f64,
    a2: f64,
    a3: f64,
) -> Result<Blade3, ()> {
    let mutual_perpendicular = !(mirror1 & mirror2 & mirror3);
    let temp_angle = (a3.sin() * a1.sin() / a2.cos()).asin();
    let temp_line = cga2d::slerp(mirror1, !mutual_perpendicular ^ !mirror1 ^ NO, temp_angle);
    let vertex_3_4 = (temp_line & mirror3).unpack_point_pair().ok_or(())?[1];
    let mirror4 = !mirror1 ^ !mirror2 ^ vertex_3_4;
    Ok(mirror4.normalize())
}
