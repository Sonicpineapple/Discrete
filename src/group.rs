use std::{collections::HashMap, fmt, ops::Mul};

/// Point acted on by the group.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Point(pub u16);
impl Point {
    pub const INIT: Self = Point(0);
}

/// Group generator.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Generator(pub u8);

/// Word in generators, applied left to right.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Word(pub Vec<Generator>);
impl Word {
    pub fn inverse(&self) -> Word {
        Word(self.0.iter().copied().rev().collect()) //TODO: Invert generators
    }
}
impl Mul for Word {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let mut vec = self.0;
        vec.extend(rhs.0);
        Word(vec)
    }
}
impl Mul for &Word {
    type Output = Word;

    fn mul(self, rhs: Self) -> Self::Output {
        let mut vec = self.0.clone();
        vec.extend(rhs.0.clone());
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
impl fmt::Display for Word {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for g in &self.0 {
            write!(f, "{} ", g.0)?;
        }
        Ok(())
    }
}

/// Permutation group multiplication table. Possibly incomplete.
#[derive(Debug, Clone)]
pub(crate) struct Group {
    point_count: u16,
    generator_count: u8,
    mul_table: HashMap<(Point, Generator), Option<Point>>,
    pub word_table: Vec<Word>,
}
impl Group {
    pub fn new(
        point_count: u16,
        generator_count: u8,
        mul_table: HashMap<(Point, Generator), Option<Point>>,
        word_table: Vec<Word>,
    ) -> Self {
        Self {
            point_count,
            mul_table,
            generator_count,
            word_table,
        }
    }

    pub fn mul_gen(&self, point: &Point, gen: &Generator) -> Option<Point> {
        self.mul_table[&(*point, *gen)]
    }

    pub fn mul_word(&self, point: &Point, word: &Word) -> Option<Point> {
        let mut result = *point;
        for gen in &word.0 {
            result = self.mul_gen(&result, gen)?;
        }
        Some(result)
    }

    pub fn point_count(&self) -> u16 {
        self.point_count
    }

    pub fn generator_count(&self) -> u8 {
        self.generator_count
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
                if let Some(q) = self.mul_gen(&Point(p), &Generator(g)) {
                    write!(f, "P{:_>2x} ", q.0)?;
                } else {
                    write!(f, "P?? ")?;
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}
