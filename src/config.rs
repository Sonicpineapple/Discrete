use crate::{
    geom::{rank_3_mirrors, rank_4_mirrors},
    group::{Group, Point},
    todd_coxeter::{get_coset_table, get_element_table},
};

pub(crate) fn parse_relation(string: &str) -> Result<Vec<u8>, ()> {
    let x: Vec<&str> = string.trim().split(';').collect();
    let rep = x[1].trim().parse();
    if rep.is_err() {
        return Err(());
    }
    let rep = rep.unwrap();

    let strings = x[0].split_ascii_whitespace().map(|b| b.parse::<u8>());
    let mut vals = vec![];
    if strings.clone().all(|m| m.is_ok()) {
        vals.extend(strings.map(|b| b.expect("How?")));
    } else {
        return Err(());
    }
    Ok((0..rep).flat_map(|_| vals.clone()).collect())
}

pub(crate) struct ViewSettings {
    pub col_scale: f32,
    pub fundamental: bool,
    pub mirrors: bool,
    pub col_tiles: bool,
    pub inverse_col: bool,
}
impl ViewSettings {
    pub fn new() -> Self {
        Self {
            col_scale: 1.,
            fundamental: true,
            mirrors: true,
            col_tiles: false,
            inverse_col: false,
        }
    }
}

pub(crate) struct Settings {
    pub depth: u32,
    pub tile_limit: u32,
    pub view_settings: ViewSettings,
    pub tiling_settings: TilingSettings,
}
impl Settings {
    pub fn new() -> Self {
        Self {
            depth: 50,
            tile_limit: 500,
            view_settings: ViewSettings::new(),
            tiling_settings: TilingSettings::new(3),
        }
    }
}

pub(crate) struct TilingSettings {
    pub rank: u8,
    pub values: Schlafli,
    pub edges: Vec<bool>,

    pub relations: Vec<String>,
    pub subgroup: Vec<u8>,
}
impl TilingSettings {
    pub fn new(rank: u8) -> Self {
        let values = Schlafli::new(rank);
        let relations = if rank == 3 {
            vec!["0 2 1;8".to_string()]
        } else if rank == 4 {
            vec!["0 2 1 0 2 1 0 1;2".to_string()]
        } else {
            vec![]
        };
        let subgroup = (0..(rank - 1)).collect();

        Self {
            rank,
            values,
            edges: vec![false, false, true, false],
            relations,
            subgroup,
        }
    }
    pub fn get_mirrors(&self) -> Option<Vec<cga2d::Blade3>> {
        Some(match self.rank {
            3 => rank_3_mirrors(self.values.0[0], self.values.0[1])?.to_vec(),
            4 => rank_4_mirrors(self.values.0[0], self.values.0[1], self.values.0[2])?.to_vec(),
            _ => todo!(),
        })
    }
    pub fn get_relations(&self) -> Result<Vec<Vec<u8>>, ()> {
        let mut rels = self.values.get_rels();
        for rel in &self.relations {
            let r = parse_relation(&rel);
            if r.is_err() {
                return Err(());
            }
            rels.push(r.unwrap());
        }
        Ok(rels)
    }
    pub fn get_puzzle_info(&self, tile_limit: u32) -> Result<PuzzleInfo, ()> {
        let rels = self.get_relations()?;
        let element_group = get_element_table(self.rank as usize, &rels, tile_limit);
        let coset_group = get_coset_table(self.rank as usize, &rels, &self.subgroup, tile_limit);

        // Inverse Element -> Coset
        let inverse_map: Vec<Option<Point>> = element_group
            .word_table
            .iter()
            .map(|word| coset_group.mul_word(Point::INIT, word.inverse()))
            .collect();

        Ok(PuzzleInfo {
            element_group,
            coset_group,
            inverse_map,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PuzzleInfo {
    pub element_group: Group,
    pub coset_group: Group,
    /// Map from a group element E to C0 * E' in the coset group
    pub inverse_map: Vec<Option<Point>>,
}

pub(crate) struct Schlafli(pub Vec<usize>);
impl Schlafli {
    pub fn new(rank: u8) -> Self {
        match rank {
            3 => Self(vec![7, 3]),
            4 => Self(vec![8, 3, 3]),
            _ => todo!(),
        }
    }

    fn get_rels(&self) -> Vec<Vec<u8>> {
        let mut rels = vec![];
        for (i, &val) in self.0.iter().enumerate() {
            for x in 0..i {
                rels.push((0..2).flat_map(|_| [x as u8, i as u8 + 1]).collect());
            }
            rels.push((0..val).flat_map(|_| [i as u8, i as u8 + 1]).collect());
        }
        rels
    }
}
