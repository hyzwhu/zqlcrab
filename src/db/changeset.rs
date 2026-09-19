//! In-memory changeset tracking for tabular grid cell edits and row deletions.

use crate::db::types::QueryValue;
use std::collections::HashMap;

/// Represents an uncommitted edit to an individual table cell.
#[derive(Debug, Clone, PartialEq)]
pub struct CellEdit {
    pub row_idx: usize,
    pub col_idx: usize,
    pub column_name: String,
    pub old_value: QueryValue,
    pub new_value: QueryValue,
}

/// Represents an uncommitted row marked for deletion.
#[derive(Debug, Clone, PartialEq)]
pub struct RowDeletion {
    pub row_idx: usize,
    pub original_row: Vec<QueryValue>,
}

/// Specifies the anchor position of an uncommitted inserted row in the grid table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertAnchor {
    /// Anchored below an existing table row (by original row index in result.rows)
    AfterRow(usize),
    /// Anchored below another inserted row (by temp_id)
    AfterInserted(usize),
    /// Anchored at the bottom of a specific page (0-indexed page)
    PageEnd(usize),
}

impl Default for InsertAnchor {
    fn default() -> Self {
        Self::PageEnd(0)
    }
}

/// Represents a newly inserted row not yet committed to the database.
#[derive(Debug, Clone, PartialEq)]
pub struct RowInsertion {
    pub temp_id: usize,
    pub values: Vec<QueryValue>,
    pub anchor: InsertAnchor,
}

/// Tracks pending local modifications (cell edits, row deletions & new rows) for an active grid.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GridChangeset {
    /// Maps (row_idx, col_idx) -> CellEdit
    pub cell_updates: HashMap<(usize, usize), CellEdit>,
    /// Maps row_idx -> RowDeletion
    pub deleted_rows: HashMap<usize, RowDeletion>,
    /// List of uncommitted new rows
    pub inserted_rows: Vec<RowInsertion>,
    /// Counter for generating unique temporary IDs for inserted rows
    pub next_insert_id: usize,
}

impl GridChangeset {
    /// Creates an empty changeset.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if there are any pending edits, deletions, or insertions.
    pub fn is_dirty(&self) -> bool {
        !self.cell_updates.is_empty() || !self.deleted_rows.is_empty() || !self.inserted_rows.is_empty()
    }

    /// Checks whether a specific cell has a pending edit.
    pub fn is_cell_dirty(&self, row_idx: usize, col_idx: usize) -> bool {
        self.cell_updates.contains_key(&(row_idx, col_idx))
    }

    /// Gets the pending cell edit if one exists.
    pub fn get_cell_edit(&self, row_idx: usize, col_idx: usize) -> Option<&CellEdit> {
        self.cell_updates.get(&(row_idx, col_idx))
    }

    /// Adds a new inserted row with default values and placement anchor, returning its unique temp_id.
    pub fn add_inserted_row(&mut self, default_values: Vec<QueryValue>, anchor: InsertAnchor) -> usize {
        let id = self.next_insert_id;
        self.next_insert_id += 1;
        self.inserted_rows.push(RowInsertion {
            temp_id: id,
            values: default_values,
            anchor,
        });
        id
    }

    /// Removes an inserted row by its temp_id.
    pub fn remove_inserted_row(&mut self, temp_id: usize) {
        if let Some(pos) = self.inserted_rows.iter().position(|r| r.temp_id == temp_id) {
            let removed = self.inserted_rows.remove(pos);
            for ins in &mut self.inserted_rows {
                if ins.anchor == InsertAnchor::AfterInserted(removed.temp_id) {
                    ins.anchor = removed.anchor;
                }
            }
        }
    }

    /// Removes an inserted row by its position/index in `inserted_rows`.
    pub fn remove_inserted_row_by_index(&mut self, index: usize) {
        if index < self.inserted_rows.len() {
            let removed = self.inserted_rows.remove(index);
            for ins in &mut self.inserted_rows {
                if ins.anchor == InsertAnchor::AfterInserted(removed.temp_id) {
                    ins.anchor = removed.anchor;
                }
            }
        }
    }

    /// Updates a cell value in an uncommitted inserted row.
    pub fn set_inserted_cell_value(&mut self, insert_idx: usize, col_idx: usize, value: QueryValue) {
        if let Some(row) = self.inserted_rows.get_mut(insert_idx) {
            if col_idx < row.values.len() {
                row.values[col_idx] = value;
            } else {
                row.values.resize(col_idx + 1, QueryValue::Null);
                row.values[col_idx] = value;
            }
        }
    }

    /// Gets a cell value from an uncommitted inserted row.
    pub fn get_inserted_cell_value(&self, insert_idx: usize, col_idx: usize) -> Option<&QueryValue> {
        self.inserted_rows.get(insert_idx).and_then(|r| r.values.get(col_idx))
    }

    /// Sets or updates a cell value. If the new value equals the original value,
    /// any pending edit for that cell is discarded.
    pub fn set_cell_value(
        &mut self,
        row_idx: usize,
        col_idx: usize,
        column_name: String,
        old_value: QueryValue,
        new_value: QueryValue,
    ) {
        if old_value == new_value {
            self.cell_updates.remove(&(row_idx, col_idx));
        } else {
            self.cell_updates.insert(
                (row_idx, col_idx),
                CellEdit {
                    row_idx,
                    col_idx,
                    column_name,
                    old_value,
                    new_value,
                },
            );
        }
    }

    /// Clears all pending cell updates and deleted rows.
    pub fn clear(&mut self) {
        self.revert_all();
    }

    /// Alias for setting a cell value.
    pub fn stage_cell_update(
        &mut self,
        row_idx: usize,
        col_idx: usize,
        col_name: impl Into<String>,
        old_value: QueryValue,
        new_value: QueryValue,
    ) {
        self.set_cell_value(row_idx, col_idx, col_name.into(), old_value, new_value);
    }

    /// Checks whether an entire row has been marked for deletion.
    pub fn is_row_deleted(&self, row_idx: usize) -> bool {
        self.deleted_rows.contains_key(&row_idx)
    }

    /// Toggles the deletion state of a row and returns the new deletion status.
    pub fn toggle_delete_row(&mut self, row_idx: usize, original_row: &[QueryValue]) -> bool {
        if self.deleted_rows.remove(&row_idx).is_some() {
            false
        } else {
            self.deleted_rows.insert(
                row_idx,
                RowDeletion {
                    row_idx,
                    original_row: original_row.to_vec(),
                },
            );
            true
        }
    }

    /// Explicitly marks a row for deletion.
    pub fn mark_row_deleted(&mut self, row_idx: usize, original_row: &[QueryValue]) {
        self.deleted_rows.insert(
            row_idx,
            RowDeletion {
                row_idx,
                original_row: original_row.to_vec(),
            },
        );
    }

    /// Unmarks a row from deletion.
    pub fn unmark_row_deleted(&mut self, row_idx: usize) {
        self.deleted_rows.remove(&row_idx);
    }

    /// Returns the effective cell value (the pending edited value if dirty, else the base value).
    pub fn get_effective_cell_value<'a>(
        &'a self,
        row_idx: usize,
        col_idx: usize,
        base: &'a QueryValue,
    ) -> &'a QueryValue {
        if let Some(edit) = self.cell_updates.get(&(row_idx, col_idx)) {
            &edit.new_value
        } else {
            base
        }
    }

    /// Reverts pending edits on a single cell.
    pub fn revert_cell(&mut self, row_idx: usize, col_idx: usize) {
        self.cell_updates.remove(&(row_idx, col_idx));
    }

    /// Reverts all modifications on a row (both cell edits and deletion markers).
    pub fn revert_row(&mut self, row_idx: usize) {
        self.deleted_rows.remove(&row_idx);
        self.cell_updates.retain(|(r, _), _| *r != row_idx);
    }

    /// Reverts all pending edits, deletions, and insertions.
    pub fn revert_all(&mut self) {
        self.cell_updates.clear();
        self.deleted_rows.clear();
        self.inserted_rows.clear();
        self.next_insert_id = 0;
    }

    /// Returns counts: (effective_cell_updates, deleted_rows_count, inserted_rows_count).
    /// Note: Cell updates on rows that are also marked for deletion are excluded from effective updates count.
    pub fn change_summary(&self) -> (usize, usize, usize) {
        let effective_updates = self
            .cell_updates
            .keys()
            .filter(|(r, _)| !self.deleted_rows.contains_key(r))
            .count();
        (
            effective_updates,
            self.deleted_rows.len(),
            self.inserted_rows.len(),
        )
    }

    /// Total number of unique modified, deleted, or newly inserted rows.
    pub fn affected_rows_count(&self) -> usize {
        let mut affected = std::collections::HashSet::new();
        for (r, _) in self.cell_updates.keys() {
            affected.insert(*r);
        }
        for r in self.deleted_rows.keys() {
            affected.insert(*r);
        }
        affected.len() + self.inserted_rows.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_changeset_lifecycle() {
        let mut cs = GridChangeset::new();
        assert!(!cs.is_dirty());
        assert_eq!(cs.change_summary(), (0, 0, 0));

        // 1. Edit a cell
        cs.set_cell_value(
            0,
            1,
            "name".to_string(),
            QueryValue::String("Alice".into()),
            QueryValue::String("Alice Cooper".into()),
        );
        assert!(cs.is_dirty());
        assert!(cs.is_cell_dirty(0, 1));
        assert_eq!(cs.change_summary(), (1, 0, 0));

        let orig = QueryValue::String("Alice".into());
        assert_eq!(
            cs.get_effective_cell_value(0, 1, &orig),
            &QueryValue::String("Alice Cooper".into())
        );

        // 2. Set value back to original -> dirty flag cleared
        cs.set_cell_value(
            0,
            1,
            "name".to_string(),
            QueryValue::String("Alice".into()),
            QueryValue::String("Alice".into()),
        );
        assert!(!cs.is_cell_dirty(0, 1));
        assert!(!cs.is_dirty());

        // 3. Mark row deleted
        let row_vals = vec![QueryValue::Int(1), QueryValue::String("Alice".into())];
        cs.toggle_delete_row(0, &row_vals);
        assert!(cs.is_dirty());
        assert!(cs.is_row_deleted(0));
        assert_eq!(cs.change_summary(), (0, 1, 0));

        // Toggle again restores it
        cs.toggle_delete_row(0, &row_vals);
        assert!(!cs.is_row_deleted(0));
        assert!(!cs.is_dirty());

        // 4. Edits on deleted rows do not double count in effective updates
        cs.set_cell_value(
            2,
            0,
            "id".to_string(),
            QueryValue::Int(10),
            QueryValue::Int(11),
        );
        cs.mark_row_deleted(2, &[QueryValue::Int(10)]);
        assert_eq!(cs.change_summary(), (0, 1, 0));
        assert_eq!(cs.affected_rows_count(), 1);

        // 5. Insert new row
        let temp_id = cs.add_inserted_row(
            vec![
                QueryValue::Null,
                QueryValue::String("David".into()),
                QueryValue::Int(28),
            ],
            InsertAnchor::PageEnd(0),
        );
        assert_eq!(temp_id, 0);
        assert!(cs.is_dirty());
        assert_eq!(cs.change_summary(), (0, 1, 1));
        assert_eq!(cs.affected_rows_count(), 2);

        // Update inserted row cell
        cs.set_inserted_cell_value(0, 1, QueryValue::String("David Miller".into()));
        assert_eq!(
            cs.get_inserted_cell_value(0, 1),
            Some(&QueryValue::String("David Miller".into()))
        );

        // Remove inserted row
        cs.remove_inserted_row(temp_id);
        assert_eq!(cs.change_summary(), (0, 1, 0));

        cs.revert_all();
        assert!(!cs.is_dirty());
        assert_eq!(cs.change_summary(), (0, 0, 0));
    }

    #[test]
    fn test_insert_anchor_hierarchy_and_reparenting() {
        let mut cs = GridChangeset::new();
        // Insert row A anchored after original row 2
        let id_a = cs.add_inserted_row(vec![QueryValue::Int(101)], InsertAnchor::AfterRow(2));
        // Insert row B anchored after inserted row A
        let id_b = cs.add_inserted_row(vec![QueryValue::Int(102)], InsertAnchor::AfterInserted(id_a));
        // Insert row C anchored after inserted row B
        let _id_c = cs.add_inserted_row(vec![QueryValue::Int(103)], InsertAnchor::AfterInserted(id_b));

        assert_eq!(cs.inserted_rows[0].anchor, InsertAnchor::AfterRow(2));
        assert_eq!(cs.inserted_rows[1].anchor, InsertAnchor::AfterInserted(id_a));
        assert_eq!(cs.inserted_rows[2].anchor, InsertAnchor::AfterInserted(id_b));

        // When row A is removed, row B should be reparented to row A's anchor (AfterRow(2))
        cs.remove_inserted_row(id_a);
        assert_eq!(cs.inserted_rows.len(), 2);
        assert_eq!(cs.inserted_rows[0].anchor, InsertAnchor::AfterRow(2));
        assert_eq!(cs.inserted_rows[1].anchor, InsertAnchor::AfterInserted(id_b));
    }
}
