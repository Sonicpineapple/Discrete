use geom::rank_4_mirrors;
use todd_coxeter::Tables;

mod geom;
mod group;
mod todd_coxeter;

fn main() {
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
