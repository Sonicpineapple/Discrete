use std::{
    collections::{HashMap, VecDeque},
    fmt,
    ops::{Index, IndexMut},
};

use crate::group::{Generator, Group, Point};

pub(crate) struct Tables {
    coset_table: CosetTable,
    relation_tables: Vec<RelationTable>,
    //subgroup_tables: Vec<Table>,
}
impl Tables {
    /// Initialise a new set of tables. Assumes subgroup generators are group generators.
    pub fn new(gen_count: usize, rels: &Vec<Vec<u8>>, subgroup: &Vec<u8>) -> Self {
        let mut out = Self {
            coset_table: CosetTable::new(gen_count),
            relation_tables: rels.iter().map(|rel| RelationTable::new(rel)).collect(),
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
        while let Some((coset, generator, result)) = new_friends.pop_front() {
            if let Some(res) = self.coset_table[coset][generator as usize] {
                if res != result {
                    // Coincidence
                    panic!("Oops, all coinkidink");
                    // let keep = min(res.0, result.0);
                    // let replace = max(res.0, result.0);
                }
            }
            // dbg!(&new_friends);
            self.coset_table[coset][generator as usize] = Some(result);
            self.coset_table[result][generator as usize] = Some(coset); // inverse

            for rel_table in &mut self.relation_tables {
                rel_table.update(&self.coset_table, &mut new_friends);
            }
        }
    }

    /// Fill in next empty coset table value with a new coset
    pub fn discover_next_unknown(&mut self) -> bool {
        let Some(i) = self.coset_table.entries.iter().position(|e| e.is_none()) else {
            return false;
        };
        let (coset, generator) = self.coset_table.unpack_index(i);
        let result = self.add_row();
        self.deduce(coset, generator as u8, result);
        // for rel in &self.relation_tables {
        //     dbg!(rel);
        // }
        //return Some(self.coset_table.to_string());
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
            mul_table.insert(
                (Point(coset.0), Generator(gen as _)),
                Point(
                    self.coset_table[coset][gen]
                        .expect("Attempted to get group for incomplete table")
                        .0,
                ),
            );
        }
        Group::new(
            self.coset_table.row_count() as u16,
            self.coset_table.gen_count as u8,
            mul_table,
        )
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
struct CosetIndex(u16);

struct CosetTable {
    entries: Vec<Option<CosetIndex>>,
    gen_count: usize,
}
impl CosetTable {
    /// Initialise a new table based on generator count.
    fn new(gen_count: usize) -> Self {
        Self {
            entries: vec![None; gen_count],
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
        CosetIndex((self.row_count() - 1) as u16)
    }
}
impl Index<CosetIndex> for CosetTable {
    type Output = [Option<CosetIndex>];

    fn index(&self, index: CosetIndex) -> &Self::Output {
        let i = index.0 as usize * self.gen_count;
        &self.entries[i..i + self.gen_count]
    }
}
impl IndexMut<CosetIndex> for CosetTable {
    fn index_mut(&mut self, index: CosetIndex) -> &mut Self::Output {
        let i = index.0 as usize * self.gen_count;
        &mut self.entries[i..i + self.gen_count]
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
                row.left_coset = result;
                row.left_rel_index += 1;
            }
            while let Some(Some(result)) = (!row.is_full())
                .then(|| coset_table[row.right_coset][self.relation[row.right_rel_index] as usize])
            {
                row.right_coset = result;
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
