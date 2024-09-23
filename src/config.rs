use std::str::FromStr;

use regex::Regex;

use crate::{
    geom::{rank_3_mirrors, rank_4_mirrors},
    group::{Group, Point},
    todd_coxeter::{get_coset_table, get_element_table},
};

pub(crate) const RELATION_PATTERN: &'static str = r"^(\d\s*(?:,\s*\d\s*)*);\s*(\d+)\s*$";
pub(crate) const SCHLAFLI_PATTERN: &'static str =
    r"^\{(\s*(?:\d+|i)(?:\s*,\s*(?:\d+|i)\s*){1,2})\}$";
pub(crate) const SUBGROUP_PATTERN: &'static str = r"^\s*(\d(?:\s*,\d)*)?\s*$";

pub(crate) fn parse_relation(string: &str) -> Result<Vec<u8>, ()> {
    let r = Regex::new(&RELATION_PATTERN).unwrap();

    if let Some(s) = r.captures(string.trim()) {
        let rel: Vec<u8> = s
            .get(1)
            .unwrap()
            .as_str()
            .split(",")
            .map(|d| d.trim().parse().expect("Guaranteed by regex"))
            .collect();
        let rep = s
            .get(2)
            .unwrap()
            .as_str()
            .parse()
            .expect("Guaranteed by regex");
        Ok((0..rep).flat_map(|_| rel.clone()).collect())
    } else {
        Err(())
    }
}

pub(crate) fn parse_subgroup(string: &str) -> Result<Vec<u8>, ()> {
    let r = Regex::new(&SUBGROUP_PATTERN).unwrap();

    if let Some(s) = r.captures(string.trim()) {
        if s.get(0).unwrap().as_str().is_empty() {
            Ok(vec![])
        } else {
            Ok(s.get(1)
                .unwrap()
                .as_str()
                .split(",")
                .map(|d| d.trim().parse().expect("Guaranteed by regex"))
                .collect())
        }
    } else {
        Err(())
    }
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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
            tiling_settings: TilingSettings::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TilingSettings {
    pub schlafli: String,
    pub relations: Vec<String>,
    pub subgroup: String,
}
impl TilingSettings {
    pub fn generate(&self) -> Result<Tiling, ()> {
        Tiling::from_settings(&self)
    }
}
impl Default for TilingSettings {
    fn default() -> Self {
        Self {
            schlafli: "{7,3}".to_string(),
            relations: vec!["0,2,1;8".to_string()],
            subgroup: "0,1".to_string(),
        }
    }
}

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
        let mut relations = schlafli.get_rels();
        let mut x = tiling_settings
            .relations
            .iter()
            .map(|r| parse_relation(r))
            .collect::<Result<_, ()>>()?;
        relations.append(&mut x);
        let subgroup = parse_subgroup(&tiling_settings.subgroup)?
            .iter()
            .map(|&x| if x <= schlafli.rank() { Ok(x) } else { Err(()) })
            .collect::<Result<_, ()>>()?;

        let mirrors = schlafli.get_mirrors()?;

        Ok(Self {
            rank: schlafli.rank(),
            schlafli: schlafli,
            mirrors,
            edges: vec![false, false, true, false],
            relations,
            subgroup,
        })
    }

    pub fn get_puzzle_info(&self, tile_limit: u32) -> Result<PuzzleInfo, ()> {
        let rels = &self.relations;
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

#[derive(Debug, Clone)]
pub(crate) struct Schlafli(pub Vec<Option<usize>>);
impl Schlafli {
    pub fn new(rank: u8) -> Self {
        match rank {
            3 => Self::from_str("{7,3}").unwrap(),
            4 => Self::from_str("{8,3,3}").unwrap(),
            _ => todo!(),
        }
    }

    fn get_rels(&self) -> Vec<Vec<u8>> {
        let mut rels = vec![];
        for (i, &val) in self.0.iter().enumerate() {
            for x in 0..i {
                rels.push((0..2).flat_map(|_| [x as u8, i as u8 + 1]).collect());
            }
            if let Some(val) = val {
                rels.push((0..val).flat_map(|_| [i as u8, i as u8 + 1]).collect());
            }
        }
        rels
    }

    fn get_mirrors(&self) -> Result<Vec<cga2d::Blade3>, ()> {
        Ok(match self.rank() {
            3 => rank_3_mirrors(self.0[0], self.0[1])?.to_vec(),
            4 => rank_4_mirrors(self.0[0], self.0[1], self.0[2])?.to_vec(),
            _ => return Err(()),
        })
    }

    pub fn rank(&self) -> u8 {
        (self.0.len() + 1) as u8
    }
}
impl FromStr for Schlafli {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let r = Regex::new(&SCHLAFLI_PATTERN).unwrap(); // Eg. {6,4}, { 7, 3,  4}, {5,i}
        if let Some(s) = r.captures(s.trim()) {
            let s = s
                .get(1)
                .expect("Guaranteed by regex")
                .as_str()
                .split(",")
                .map(|d| match d.trim() {
                    "i" => None,
                    x => Some(x.parse().expect("Guaranteed by regex")),
                })
                .collect();
            Ok(Self(s))
        } else {
            Err(())
        }
    }
}
