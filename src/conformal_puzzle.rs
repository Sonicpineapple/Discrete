use std::sync::Arc;

use crate::{
    group::{Generator, Point, Word},
    puzzle::{GripSignature, Puzzle},
    tiling::{QuotientGroup, Tiling},
};
use cga2d::prelude::*;

pub(crate) struct ConformalPuzzle {
    pub puzzle: Puzzle,
    pub tiling: Arc<Tiling>,
    pub quotient_group: Arc<QuotientGroup>,
    pub base_twists: Vec<Word>,
    pub cut_circles: Vec<cga2d::Blade3>,
    pub cut_map: Vec<Option<usize>>,
    pub editor: Option<PuzzleEditor>,
}
impl ConformalPuzzle {
    // pub fn new(tiling: Arc<Tiling>, tile_limit: u32) -> Result<Self, ()> {
    //     let puzzle_info = tiling.get_puzzle_info(tile_limit)?;
    //     let piece_types = vec![
    //         GripSignature(vec![
    //             Point::INIT,
    //             puzzle_info
    //                 .coset_group
    //                 .mul_word(&Point::INIT, &Word(vec![Generator(0), Generator(2)]))
    //                 .unwrap(),
    //         ]),
    //         GripSignature(vec![
    //             Point::INIT,
    //             puzzle_info
    //                 .coset_group
    //                 .mul_word(&Point::INIT, &Word(vec![Generator(1), Generator(2)]))
    //                 .unwrap(),
    //             puzzle_info
    //                 .coset_group
    //                 .mul_word(
    //                     &Point::INIT,
    //                     &Word(vec![Generator(1), Generator(2), Generator(1), Generator(2)]),
    //                 )
    //                 .unwrap(),
    //         ]),
    //     ];
    //     let puzzle = Puzzle::new(
    //         puzzle_info.element_group,
    //         puzzle_info.coset_group,
    //         piece_types,
    //     )?;
    //     // let puzzle = Puzzle::new_anticore_only(
    //     //     puzzle_info.element_group.clone(),
    //     //     puzzle_info.coset_group.clone(),
    //     // );
    //     let cut_map = (0..1 << cut_circles.len())
    //         .map(|i| if i < 2 { Some(i) } else { None })
    //         .collect();

    //     let inverse_map = puzzle_info.inverse_map;
    //     let base_twists = vec![Word(vec![Generator(0), Generator(1)])];

    //     Ok(Self {
    //         puzzle,
    //         tiling,
    //         base_twists,
    //         inverse_map,
    //         cut_circles,
    //         cut_map,
    //         editor: None,
    //     })
    // }

    fn from_definition(definition: &PuzzleDefinition) -> Result<Self, ()> {
        let quotient_group = definition.quotient_group.clone();
        let puzzle = Puzzle::new(
            quotient_group.element_group.clone(),
            quotient_group.tile_group.clone(),
            definition.piece_types.clone(),
        )?;
        let base_twists = vec![Word(vec![Generator(0), Generator(1)])];
        Ok(Self {
            puzzle,
            tiling: definition.tiling.clone(),
            quotient_group,
            base_twists,
            cut_circles: definition.cut_circles.clone(),
            cut_map: definition.cut_map.clone(),
            editor: None,
        })
    }

    pub fn apply_move(
        &mut self,
        attitude: Word,
        twist: usize,
        mut inverse: bool,
    ) -> Result<(), ()> {
        if attitude.0.len() % 2 == 1 {
            inverse = !inverse;
        }
        let grip = self
            .puzzle
            .grip_group
            .mul_word(&Point::INIT, &attitude.inverse())
            .ok_or(())?;
        let twist = &mut self.base_twists[twist].clone();
        if inverse {
            *twist = twist.inverse();
        }
        let turn = &attitude * twist * attitude.inverse();
        self.puzzle.apply_move(&grip, &turn)
    }

    pub fn add_piece_types(&mut self, piece_types: Vec<GripSignature>) -> Result<(), ()> {
        let mut types = self.puzzle.piece_types.clone();
        for t in &piece_types {
            if !types.contains(&t) {
                types.push(t.clone());
            }
        }
        self.puzzle = Puzzle::new(
            self.puzzle.elem_group.clone(),
            self.puzzle.grip_group.clone(),
            piece_types,
        )?;
        Ok(())
    }

    pub fn regenerate_puzzle(&mut self) -> Result<(), ()> {
        self.puzzle = Puzzle::new(
            self.puzzle.elem_group.clone(),
            self.puzzle.grip_group.clone(),
            self.puzzle.piece_types.clone(),
        )?;
        Ok(())
    }

    pub fn get_cut_mask(&self, point: cga2d::Blade1) -> usize {
        self.cut_circles.iter().enumerate().fold(0, |m, (i, c)| {
            if !(*c ^ point) > 0. {
                m + (1 << i)
            } else {
                m
            }
        })
    }
}

/// Intermediate information for editing piece types
pub struct PuzzleEditor {
    pub active_piece_type: Option<usize>,
    pub puzzle_def: PuzzleDefinition,
}
impl PuzzleEditor {
    pub fn new(puzzle_def: PuzzleDefinition) -> Self {
        Self {
            active_piece_type: None,
            puzzle_def,
        }
    }
}

pub struct PuzzleDefinition {
    pub tiling: Arc<Tiling>,
    pub quotient_group: Arc<QuotientGroup>,
    pub piece_types: Vec<GripSignature>,
    pub cut_circles: Vec<cga2d::Blade3>,
    pub cut_map: Vec<Option<usize>>,
}
impl PuzzleDefinition {
    pub fn new(tiling: Arc<Tiling>, quotient_group: Arc<QuotientGroup>) -> Self {
        let piece_types = vec![GripSignature(vec![Point::INIT])];

        let ms = &tiling.mirrors;
        let p = ms[0] & ms[1];
        let cut_circle = -cga2d::slerp(
            ms[2],
            -ms[2].connect(p).connect(p),
            std::f64::consts::PI / 6.,
        );

        let cut_circles = vec![cut_circle, (ms[1] * ms[0]).sandwich(cut_circle)];
        let cut_map = (0..1 << cut_circles.len())
            .map(|i| if i < 1 { Some(i) } else { None })
            .collect();

        Self {
            tiling,
            quotient_group,
            piece_types,
            cut_circles,
            cut_map,
        }
    }

    pub fn generate_puzzle(&self) -> Result<ConformalPuzzle, ()> {
        ConformalPuzzle::from_definition(self)
    }

    pub fn get_cut_mask(&self, point: cga2d::Blade1) -> usize {
        self.cut_circles.iter().enumerate().fold(0, |m, (i, c)| {
            if !(*c ^ point) > 0. {
                m + (1 << i)
            } else {
                m
            }
        })
    }
}
