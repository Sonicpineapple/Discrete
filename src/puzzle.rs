use crate::group::{Group, Point, Word};

pub(crate) struct Puzzle {
    pub elem_group: Group,
    pub grip_group: Group,
    // subgroup: Group,
    /// Pieces will be drawn based on the position of the seed signature
    pub piece_types: Vec<GripSignature>,
    pub pieces: Vec<Piece>,
}
impl Puzzle {
    pub fn new_anticore_only(elem_group: Group, grip_group: Group) -> Self {
        let pieces = vec![Piece {
            attitude: Point::INIT,
            grips: GripSignature((0..grip_group.point_count()).map(|q| Point(q)).collect()),
        }];
        Self {
            elem_group,
            grip_group,
            piece_types: vec![],
            pieces,
        }
    }

    pub fn new(
        elem_group: Group,
        grip_group: Group,
        piece_types: Vec<GripSignature>,
    ) -> Result<Self, ()> {
        let mut sigs = vec![];
        for sig in &piece_types {
            for word in (0..elem_group.point_count()).map(|i| &elem_group.word_table[i as usize]) {
                let new_sig = Self::free_transform_signature(&sig, &grip_group, word)?;
                if !sigs.contains(&new_sig) {
                    sigs.push(new_sig);
                }
            }
        }
        let pieces = sigs
            .iter()
            .map(move |sig| Piece {
                attitude: Point::INIT,
                grips: sig.clone(),
            })
            .collect();
        Ok(Self {
            elem_group,
            grip_group,
            piece_types,
            pieces,
        })
    }

    pub fn apply_move(&mut self, grip: &Point, word: &Word) -> Result<(), ()> {
        for piece in &mut self.pieces {
            if piece.grips.contains(grip) {
                piece.attitude = self.elem_group.mul_word(&piece.attitude, &word).ok_or(())?;
                for g in &mut piece.grips.0 {
                    *g = self.grip_group.mul_word(g, &word).ok_or(())?
                }
            }
        }
        Ok(())
    }

    pub fn free_transform_signature(
        sig: &GripSignature,
        grip_group: &Group,
        word: &Word,
    ) -> Result<GripSignature, ()> {
        let mut out = sig.clone();
        for g in &mut out.0 {
            *g = grip_group.mul_word(&g, word).ok_or(())?
        }
        Ok(out)
    }

    pub fn transform_signature(
        &self,
        sig: &GripSignature,
        word: &Word,
    ) -> Result<GripSignature, ()> {
        Self::free_transform_signature(sig, &self.grip_group, word)
    }

    pub fn find_piece(&self, index: GripSignature) -> Option<&Piece> {
        self.pieces.iter().find(|p| p.grips == index)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Piece {
    /// Group element
    pub attitude: Point,
    /// Set of cosets
    pub grips: GripSignature,
}

#[derive(Debug, Clone)]
pub(crate) struct GripSignature(pub Vec<Point>);
impl GripSignature {
    pub const CORE: Self = Self(vec![]);

    pub fn contains(&self, grip: &Point) -> bool {
        self.0.contains(grip)
    }
}
impl PartialEq for GripSignature {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len() && self.0.iter().all(|g| other.0.contains(g))
    }
}
