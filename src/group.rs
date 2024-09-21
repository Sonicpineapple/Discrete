use std::{collections::HashMap, fmt, ops::Mul};

/// Point acted on by the group.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Point(pub u16);

/// Group generator.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Generator(pub u8);

/// Word in generators, applied left to right.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Word(Vec<Generator>);
impl Mul for Word {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let mut vec = self.0;
        vec.extend(rhs.0);
        Word(vec)
    }
}
impl Mul<Word> for Generator {
    type Output = Word;

    fn mul(self, rhs: Word) -> Self::Output {
        Word(vec![self]) * rhs
    }
}
impl Mul<Generator> for Word {
    type Output = Word;

    fn mul(self, rhs: Generator) -> Self::Output {
        self * Word(vec![rhs])
    }
}

/// Permutation group multiplication table.
pub(crate) struct Group {
    point_count: u16,
    generator_count: u8,
    mul_table: HashMap<(Point, Generator), Point>,
}
impl Group {
    pub fn new(
        point_count: u16,
        generator_count: u8,
        mul_table: HashMap<(Point, Generator), Point>,
    ) -> Self {
        Self {
            point_count,
            mul_table,
            generator_count,
        }
    }

    pub fn mul_gen(&self, point: Point, gen: Generator) -> Point {
        self.mul_table[&(point, gen)]
    }

    pub fn mul_word(&self, point: Point, word: Word) -> Point {
        let mut result = point;
        for gen in word.0 {
            result = self.mul_gen(point, gen);
        }
        result
    }
}
impl fmt::Display for Group {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Points: {}", self.point_count)?;
        writeln!(f, "Generators: {}", self.generator_count)?;
        write!(f, "P\\G ")?;
        for g in 0..self.generator_count {
            write!(f, "G{g:_>2x} ")?;
        }
        writeln!(f)?;
        for p in 0..self.point_count {
            write!(f, "P{p:_>2x} ")?;
            for g in 0..self.generator_count {
                write!(f, "P{:_>2x} ", self.mul_gen(Point(p), Generator(g)).0)?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}
