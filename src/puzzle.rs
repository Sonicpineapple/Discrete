use crate::group::{Group, Point, Word};

pub(crate) struct Puzzle {
    elem_group: Group,
    grip_group: Group,
    // subgroup: Group,
    pub pieces: Vec<Piece>,
}
impl Puzzle {
    pub fn new_anticore_only(elem_group: Group, grip_group: Group) -> Self {
        let pieces = vec![Piece {
            attitude: Point::INIT,
            grips: (0..grip_group.point_count()).map(|q| Point(q)).collect(),
        }];
        Self {
            elem_group,
            grip_group,
            pieces,
        }
    }

    pub fn apply_move(&mut self, grip: &Point, word: &Word) -> Result<(), ()> {
        for piece in &mut self.pieces {
            if piece.grips.contains(grip) {
                piece.attitude = self.elem_group.mul_word(&piece.attitude, &word).ok_or(())?;
                for g in &mut piece.grips {
                    *g = self.grip_group.mul_word(g, &word).ok_or(())?
                }
            }
        }
        Ok(())
    }
}

pub(crate) struct Piece {
    /// Group element
    pub attitude: Point,
    /// Set of cosets
    pub grips: Vec<Point>,
}
