use std::{
    collections::{HashMap, VecDeque},
    fmt,
    ops::{Index, IndexMut},
};

use crate::group::{Generator, Group, Point, Word};

pub(crate) fn get_element_table(gen_count: usize, rels: &Vec<Vec<u8>>, limit: u32) -> Group {
    get_coset_table(gen_count, rels, &vec![], limit)
}

pub(crate) fn get_coset_table(
    gen_count: usize,
    rels: &Vec<Vec<u8>>,
    subgroup: &Vec<u8>,
    limit: u32,
) -> Group {
    let mut tables = Tables::new(gen_count, rels, subgroup);
    let mut i = 0;
    while (i < limit) && tables.discover_next_unknown() {
        i += 1
    }
    tables.coset_group()
}

pub(crate) struct Tables {
    coset_table: CosetTable,
    relation_tables: Vec<RelationTable>,
    word_table: WordTable,
    //subgroup_tables: Vec<Table>,
}
impl Tables {
    /// Initialise a new set of tables. Assumes subgroup generators are group generators.
    pub fn new(gen_count: usize, rels: &Vec<Vec<u8>>, subgroup: &Vec<u8>) -> Self {
        let mut out = Self {
            coset_table: CosetTable::new(gen_count),
            relation_tables: rels.iter().map(|rel| RelationTable::new(rel)).collect(),
            word_table: WordTable::new(),
            //subgroup_tables: subgroup.iter().map(|gen| Table::new(gen.len())).collect(),
        };
        for &sub_gen in subgroup {
            out.deduce(CosetIndex(0), sub_gen, CosetIndex(0));
            // println!("{}", out.coset_table.to_string())
        }
        out
    }

    /// Fill in tables based on a new result.
    fn deduce(&mut self, coset: CosetIndex, generator: u8, result: CosetIndex) {
        let mut new_friends = VecDeque::from(vec![(coset, generator, result)]);
        while let Some((mut coset, generator, mut result)) = new_friends.pop_front() {
            coset = self.coset_table.redirect_index(coset);
            result = self.coset_table.redirect_index(result);
            if let Some(res) = self.coset_table[coset][generator as usize] {
                if res != result {
                    // Coincidence
                    let replace = res.max(result);
                    result = res.min(result);

                    self.resolve_coincidence(result, replace);
                }
            }

            self.coset_table[coset][generator as usize] =
                Some(self.coset_table.redirect_index(result));
            self.coset_table[result][generator as usize] =
                Some(self.coset_table.redirect_index(coset)); // inverse

            for rel_table in &mut self.relation_tables {
                rel_table.update(&self.coset_table, &mut new_friends);
            }
        }
    }

    /// Fix a duplicate result.
    fn resolve_coincidence(&mut self, keep: CosetIndex, replace: CosetIndex) {
        self.coset_table.tombstones[replace.0 as usize] = Some(keep);

        let replace_index = |c: CosetIndex| match c.cmp(&replace) {
            std::cmp::Ordering::Equal => keep,
            _ => c,
        };

        self.coset_table
            .tombstones
            .iter_mut()
            .for_each(|i| *i = i.map(replace_index));
        self.coset_table
            .entries
            .iter_mut()
            .for_each(|i| *i = i.map(replace_index));
        for rel_table in &mut self.relation_tables {
            for row in &mut rel_table.rows {
                row.left_coset = replace_index(row.left_coset);
                row.right_coset = replace_index(row.right_coset);
            }
        }

        (0..self.coset_table.gen_count).for_each(|g| {
            if let Some(res) = self.coset_table[replace][g] {
                self.deduce(keep, g as u8, res);
            }
        });
    }

    /// Fill in next empty coset table value with a new coset
    pub fn discover_next_unknown(&mut self) -> bool {
        let Some(i) = self.coset_table.entries.iter().position(|e| e.is_none()) else {
            return false;
        };
        let (coset, generator) = self.coset_table.unpack_index(i);
        let result = self.add_row();
        let new_word = self.word_table[coset].clone() * Generator(generator as u8);
        self.word_table.push(new_word);
        self.deduce(coset, generator as u8, result);

        let mut fresh_indices = 0..;
        let index_replacements: Vec<CosetIndex> = self
            .coset_table
            .tombstones
            .iter()
            .map(|replacement| {
                replacement.unwrap_or_else(|| CosetIndex(fresh_indices.next().unwrap()))
            })
            .collect();
        let replace_index = |c: CosetIndex| index_replacements[c.0 as usize];

        // Reindex everyone down
        self.coset_table
            .entries
            .iter_mut()
            .for_each(|i| *i = i.map(replace_index));
        for rel_table in &mut self.relation_tables {
            for row in &mut rel_table.rows {
                row.left_coset = replace_index(row.left_coset);
                row.right_coset = replace_index(row.right_coset);
            }
            rel_table.remove_redirected(&self.coset_table.tombstones);
        }
        self.word_table
            .remove_redirected(&self.coset_table.tombstones);

        // Remove everyone replaced and throw out old tombstones
        self.coset_table.remove_redirected();

        return true;
    }

    /// Initialise a new row for a new coset, returning the index of that coset.
    fn add_row(&mut self) -> CosetIndex {
        let index = self.coset_table.add_row();
        for rel_table in &mut self.relation_tables {
            rel_table.add_row(index);
        }
        index
    }

    pub fn coset_group(&self) -> Group {
        let mut mul_table = HashMap::new();
        for (i, e) in self.coset_table.entries.iter().enumerate() {
            let (coset, gen) = self.coset_table.unpack_index(i);
            mul_table.insert((Point(coset.0), Generator(gen as _)), e.map(|e| Point(e.0)));
        }
        Group::new(
            self.coset_table.row_count() as u16,
            self.coset_table.gen_count as u8,
            mul_table,
            self.word_table.words.clone(),
        )
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct CosetIndex(u16);

struct CosetTable {
    entries: Vec<Option<CosetIndex>>,
    tombstones: Vec<Option<CosetIndex>>,
    gen_count: usize,
}
impl CosetTable {
    /// Initialise a new table based on generator count.
    fn new(gen_count: usize) -> Self {
        Self {
            entries: vec![None; gen_count],
            tombstones: vec![None; gen_count],
            gen_count,
        }
    }

    /// Get number of cosets.
    fn row_count(&self) -> usize {
        self.entries.len() / self.gen_count
    }

    /// Convert a linear index into (row,column) form based on column count.
    fn unpack_index_with_count(index: usize, col_count: usize) -> (CosetIndex, usize) {
        let coset_index = CosetIndex((index / col_count) as u16);
        let column = index % col_count;
        (coset_index, column)
    }

    /// Convert a linear index into (row,column) form based on this table.
    fn unpack_index(&self, index: usize) -> (CosetIndex, usize) {
        Self::unpack_index_with_count(index, self.gen_count)
    }

    /// Initialise a new row for a new coset, returning the index of that coset.
    fn add_row(&mut self) -> CosetIndex {
        self.entries.extend((0..self.gen_count).map(|_| None));
        self.tombstones.push(None);
        CosetIndex((self.row_count() - 1) as u16)
    }

    fn remove(&mut self, index: CosetIndex) -> Vec<Option<CosetIndex>> {
        let range = self.row_range(index);
        self.entries.drain(range).collect()
    }

    fn row_range(&self, index: CosetIndex) -> std::ops::Range<usize> {
        let i = index.0 as usize * self.gen_count;
        i..i + self.gen_count
    }

    /// Cascade a coset through any reindexings
    fn redirect_index(&self, mut index: CosetIndex) -> CosetIndex {
        while let Some(redirect) = self.tombstones[index.0 as usize] {
            index = redirect;
        }
        index
    }

    /// Remove rows for cosets that have been reindexed
    fn remove_redirected(&mut self) {
        let rows = self.row_count();
        let Self {
            entries,
            tombstones,
            gen_count,
        } = self;

        for t in (0..rows).rev() {
            if tombstones[t].is_some() {
                let i = t * *gen_count;
                let range = i..i + *gen_count;
                entries.drain(range);
            }
        }
        tombstones.retain(|t| t.is_none());
    }
}
impl Index<CosetIndex> for CosetTable {
    type Output = [Option<CosetIndex>];

    fn index(&self, index: CosetIndex) -> &Self::Output {
        &self.entries[self.row_range(self.redirect_index(index))]
    }
}
impl IndexMut<CosetIndex> for CosetTable {
    fn index_mut(&mut self, index: CosetIndex) -> &mut Self::Output {
        let range = self.row_range(self.redirect_index(index));
        &mut self.entries[range]
    }
}
impl fmt::Display for CosetTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Columns: {}", self.gen_count)?;
        for i in 0..self.row_count() {
            write!(f, "C{}:", i)?;
            for &e in self.entries[i * self.gen_count..(i + 1) * self.gen_count].iter() {
                if let Some(index) = e {
                    write!(f, "C{}, ", index.0)?;
                } else {
                    write!(f, "??, ")?;
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct RelationTable {
    relation: Vec<u8>,
    rows: Vec<RelationTableRow>,
}
impl RelationTable {
    /// Initialise a new table based on a group relation.
    fn new(relation: &Vec<u8>) -> Self {
        Self {
            relation: relation.clone(),
            rows: vec![RelationTableRow::new(relation.len(), CosetIndex(0))],
        }
    }

    /// Update this table with a new fact, cascading results found.
    fn update(
        &mut self,
        coset_table: &CosetTable,
        new_facts: &mut VecDeque<(CosetIndex, u8, CosetIndex)>,
    ) {
        for row in &mut self.rows {
            if row.is_full() {
                continue;
            }
            while let Some(Some(result)) = (!row.is_full())
                .then(|| coset_table[row.left_coset][self.relation[row.left_rel_index] as usize])
            {
                row.left_coset = coset_table.redirect_index(result);
                row.left_rel_index += 1;
            }
            while let Some(Some(result)) = (!row.is_full())
                .then(|| coset_table[row.right_coset][self.relation[row.right_rel_index] as usize])
            {
                row.right_coset = coset_table.redirect_index(result);
                row.right_rel_index -= 1;
            }
            if row.is_full() {
                new_facts.push_back((
                    row.left_coset,
                    self.relation[row.left_rel_index],
                    row.right_coset,
                ));
            }
        }
    }

    /// Initialise a new row for a given coset.
    fn add_row(&mut self, index: CosetIndex) {
        self.rows
            .push(RelationTableRow::new(self.relation.len(), index));
    }

    /// Remove rows for cosets that have been reindexed
    fn remove_redirected(&mut self, tombstones: &Vec<Option<CosetIndex>>) {
        for t in (0..tombstones.len()).rev() {
            if tombstones[t].is_some() {
                self.rows.remove(t);
            }
        }
    }
}
impl Index<CosetIndex> for RelationTable {
    type Output = RelationTableRow;

    fn index(&self, index: CosetIndex) -> &Self::Output {
        &self.rows[index.0 as usize]
    }
}
impl IndexMut<CosetIndex> for RelationTable {
    fn index_mut(&mut self, index: CosetIndex) -> &mut Self::Output {
        &mut self.rows[index.0 as usize]
    }
}

#[derive(Debug, Clone)]
struct RelationTableRow {
    left_coset: CosetIndex,
    right_coset: CosetIndex,
    left_rel_index: usize,
    right_rel_index: usize,
}
impl RelationTableRow {
    /// Initialise a new row.
    fn new(length: usize, coset: CosetIndex) -> Self {
        Self {
            left_coset: coset,
            right_coset: coset,
            left_rel_index: 0,
            right_rel_index: length - 1,
        }
    }

    /// Whether this row is completely filled in.
    fn is_full(&self) -> bool {
        self.left_rel_index >= self.right_rel_index
    }
}

#[derive(Debug, Clone)]
struct WordTable {
    words: Vec<Word>,
}
impl WordTable {
    fn new() -> Self {
        Self {
            words: vec![Word(vec![])],
        }
    }

    fn push(&mut self, word: Word) {
        self.words.push(word);
    }

    fn remove_redirected(&mut self, tombstones: &Vec<Option<CosetIndex>>) {
        for t in (0..tombstones.len()).rev() {
            if tombstones[t].is_some() {
                self.words.remove(t);
            }
        }
    }
}
impl Index<CosetIndex> for WordTable {
    type Output = Word;

    fn index(&self, index: CosetIndex) -> &Self::Output {
        &self.words[index.0 as usize]
    }
}
impl IndexMut<CosetIndex> for WordTable {
    fn index_mut(&mut self, index: CosetIndex) -> &mut Self::Output {
        &mut self.words[index.0 as usize]
    }
}
