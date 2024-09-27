use std::str::FromStr;

use crate::{
    config::{parse_relation, parse_subgroup, Schlafli, TilingSettings},
    group::{Group, Point},
    todd_coxeter::{get_coset_table, get_element_table},
};

#[derive(Debug, Clone)]
pub(crate) struct Tiling {
    pub rank: u8,
    pub schlafli: Schlafli,
    pub mirrors: Vec<cga2d::Blade3>,
    pub edges: Vec<bool>,

    pub relations: Vec<Vec<u8>>,
    pub subgroup: Vec<u8>,
}
impl Tiling {
    pub fn from_settings(tiling_settings: &TilingSettings) -> Result<Self, ()> {
        let schlafli = Schlafli::from_str(&tiling_settings.schlafli)?;
        let rank = schlafli.rank();
        let mut relations = schlafli.get_rels();
        let mut x: Vec<Vec<u8>> = tiling_settings
            .relations
            .iter()
            .map(|r| parse_relation(r))
            .collect::<Result<_, ()>>()?;
        if !x.iter().all(|r| r.iter().all(|&g| g < rank)) {
            return Err(());
        };
        relations.append(&mut x);
        let subgroup = parse_subgroup(&tiling_settings.subgroup)?
            .iter()
            .map(|&x| if x <= schlafli.rank() { Ok(x) } else { Err(()) })
            .collect::<Result<_, ()>>()?;

        let mut edges = vec![true; 4];
        for &i in &subgroup {
            edges[i as usize] = false;
        }

        let mirrors = schlafli.get_mirrors()?;

        Ok(Self {
            rank,
            schlafli,
            mirrors,
            edges,
            relations,
            subgroup,
        })
    }

    pub fn get_quotient_group(&self, tile_limit: u32) -> Result<QuotientGroup, ()> {
        let rels = &self.relations;
        let element_group = get_element_table(self.rank as usize, &rels, tile_limit);
        let tile_group = get_coset_table(self.rank as usize, &rels, &self.subgroup, tile_limit);

        // Inverse Element -> Coset
        let inverse_map: Vec<Option<Point>> = element_group
            .word_table
            .iter()
            .map(|word| tile_group.mul_word(&Point::INIT, &word.inverse()))
            .collect();

        Ok(QuotientGroup {
            element_group,
            tile_group,
            inverse_map,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct QuotientGroup {
    pub element_group: Group,
    pub tile_group: Group,
    /// Map from a group element E to C0 * E' in the coset group
    pub inverse_map: Vec<Option<Point>>,
}
