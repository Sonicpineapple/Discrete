use std::ops::Index;

use crate::{
    config::{PuzzleInfo, Tiling},
    group::{Generator, Point, Word},
    puzzle::{GripSignature, Puzzle},
};
use cga2d::prelude::*;

pub(crate) struct ConformalPuzzle {
    pub puzzle: Puzzle,
    pub tiling: Tiling,
    pub base_twists: Vec<Word>,
    pub inverse_map: Vec<Option<Point>>,
    pub cut_circles: Vec<cga2d::Blade3>,
    pub cut_map: Vec<Option<usize>>,
    pub editor: Option<PuzzleEditor>,
}
impl ConformalPuzzle {
    pub fn new(tiling: Tiling, tile_limit: u32) -> Result<Self, ()> {
        let puzzle_info = tiling.get_puzzle_info(tile_limit)?;
        let piece_types = vec![
            GripSignature(vec![
                Point::INIT,
                puzzle_info
                    .coset_group
                    .mul_word(&Point::INIT, &Word(vec![Generator(0), Generator(2)]))
                    .unwrap(),
            ]),
            GripSignature(vec![
                Point::INIT,
                puzzle_info
                    .coset_group
                    .mul_word(&Point::INIT, &Word(vec![Generator(1), Generator(2)]))
                    .unwrap(),
                puzzle_info
                    .coset_group
                    .mul_word(
                        &Point::INIT,
                        &Word(vec![Generator(1), Generator(2), Generator(1), Generator(2)]),
                    )
                    .unwrap(),
            ]),
        ];
        let puzzle = Puzzle::new(
            puzzle_info.element_group,
            puzzle_info.coset_group,
            piece_types,
        )?;
        // let puzzle = Puzzle::new_anticore_only(
        //     puzzle_info.element_group.clone(),
        //     puzzle_info.coset_group.clone(),
        // );

        let cut_circles = puzzle_info.cut_circles;
        let cut_map = (0..1 << cut_circles.len())
            .map(|i| if i < 2 { Some(i) } else { None })
            .collect();

        let inverse_map = puzzle_info.inverse_map;
        let base_twists = vec![Word(vec![Generator(0), Generator(1)])];

        Ok(Self {
            puzzle,
            tiling,
            base_twists,
            inverse_map,
            cut_circles,
            cut_map,
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

    pub fn set_editor(&mut self, piece_type: usize) {
        self.editor = Some(PuzzleEditor {
            piece_type,
            grips: self.puzzle.piece_types[piece_type].0.clone(),
            cut_mask: self.cut_map[piece_type],
        });
    }

    pub fn apply_editor(&mut self) -> Result<(), ()> {
        let Self {
            puzzle,
            cut_map,
            editor,
            ..
        } = self;
        if let Some(editor) = editor {
            puzzle.piece_types[editor.piece_type].0 = editor.grips.clone();
            if let Some(cut_mask) = editor.cut_mask {
                if cut_map[cut_mask].is_some() {
                    cut_map[cut_mask] = None;
                } else {
                    cut_map[cut_mask] = Some(editor.piece_type);
                }
            }
        }
        *editor = None;
        self.regenerate_puzzle()
    }
}

/// Intermediate information for editing piece types
pub struct PuzzleEditor {
    pub piece_type: usize,
    pub grips: Vec<Point>,
    pub cut_mask: Option<usize>,
}
