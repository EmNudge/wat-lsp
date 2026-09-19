//! Coordinate conversion for one immutable source snapshot.
//!
//! Feature request positions are UTF-16. Tree-sitter points and core ranges
//! derived from the tree have UTF-8 byte columns. Convert those ranges exactly
//! once, at the protocol boundary. Neither conversion may cross a line ending.

use super::types::{Position, Range};

pub struct TextIndex<'a> {
    source: &'a str,
    starts: Vec<usize>,
    // Cumulative UTF-8 minus UTF-16 width after each non-ASCII scalar. Outgoing
    // ranges can then be converted without rescanning a long line per symbol.
    utf16_adjustments: Vec<(usize, usize)>,
}

impl<'a> TextIndex<'a> {
    pub fn new(source: &'a str) -> Self {
        let mut starts = vec![0];
        let mut utf16_adjustments = Vec::new();
        let mut excess = 0;
        for (byte, ch) in source.char_indices() {
            if ch == '\n' {
                starts.push(byte + 1);
            }
            if !ch.is_ascii() {
                excess += ch.len_utf8() - ch.len_utf16();
                utf16_adjustments.push((byte + ch.len_utf8(), excess));
            }
        }
        Self {
            source,
            starts,
            utf16_adjustments,
        }
    }

    fn utf16_excess_at(&self, byte: usize) -> usize {
        let count = self
            .utf16_adjustments
            .partition_point(|&(end, _)| end <= byte);
        count
            .checked_sub(1)
            .map_or(0, |i| self.utf16_adjustments[i].1)
    }

    fn line_bounds(&self, line: usize) -> Option<(usize, usize)> {
        let start = *self.starts.get(line)?;
        let mut end = self
            .starts
            .get(line + 1)
            .map_or(self.source.len(), |next| next - 1);
        if end > start && self.source.as_bytes()[end - 1] == b'\r' {
            end -= 1;
        }
        Some((start, end))
    }

    pub fn line(&self, line: usize) -> Option<&'a str> {
        let (start, end) = self.line_bounds(line)?;
        Some(&self.source[start..end])
    }

    /// UTF-16 position to an absolute byte offset. Out-of-document lines map to
    /// EOF, columns stop before CRLF/LF, and split surrogate pairs round down.
    pub fn utf16_to_byte(&self, position: Position) -> usize {
        let Some((start, end)) = self.line_bounds(position.line as usize) else {
            return self.source.len();
        };
        start + utf16_column_to_byte(&self.source[start..end], position.character)
    }

    /// Absolute byte offset to a UTF-16 position, clamped to a scalar boundary.
    pub fn byte_to_utf16(&self, byte: usize) -> Position {
        let byte = self.floor_boundary(byte);
        let line = self.starts.partition_point(|&start| start <= byte) - 1;
        let (start, end) = self.line_bounds(line).unwrap();
        let byte = byte.min(end);
        let units = byte - start - (self.utf16_excess_at(byte) - self.utf16_excess_at(start));
        Position::new(line as u32, units as u32)
    }

    /// Absolute byte offset to a tree-sitter point (UTF-8 column). Unlike client
    /// positions, points inside CRLF retain their actual byte column.
    pub fn byte_to_point(&self, byte: usize) -> Position {
        let byte = self.floor_boundary(byte);
        let line = self.starts.partition_point(|&start| start <= byte) - 1;
        Position::new(line as u32, (byte - self.starts[line]) as u32)
    }

    pub fn point_to_byte(&self, position: Position) -> usize {
        let Some((start, end)) = self.line_bounds(position.line as usize) else {
            return self.source.len();
        };
        self.floor_boundary(start.saturating_add(position.character as usize).min(end))
    }

    pub fn point_to_utf16(&self, position: Position) -> Position {
        self.byte_to_utf16(self.point_to_byte(position))
    }

    pub fn range_to_utf16(&self, range: Range) -> Range {
        let start = self.point_to_byte(range.start);
        let end = self.point_to_byte(range.end).max(start);
        Range::new(self.byte_to_utf16(start), self.byte_to_utf16(end))
    }

    fn floor_boundary(&self, byte: usize) -> usize {
        let mut byte = byte.min(self.source.len());
        while !self.source.is_char_boundary(byte) {
            byte -= 1;
        }
        byte
    }
}

pub fn utf16_column_to_byte(line: &str, column: u32) -> usize {
    let mut remaining = column as usize;
    for (byte, ch) in line.char_indices() {
        if remaining < ch.len_utf16() {
            return byte;
        }
        remaining -= ch.len_utf16();
    }
    line.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_positions_round_trip_at_every_scalar_boundary() {
        for source in ["", "ascii", "é漢😀z", "é漢😀\r\nz\n", "\n\n", "😀\n"] {
            let index = TextIndex::new(source);
            for byte in (0..=source.len()).filter(|&i| source.is_char_boundary(i)) {
                if source.as_bytes().get(byte) == Some(&b'\r')
                    || (source.as_bytes().get(byte) == Some(&b'\n')
                        && byte > 0
                        && source.as_bytes()[byte - 1] == b'\r')
                {
                    continue;
                }
                assert_eq!(
                    index.utf16_to_byte(index.byte_to_utf16(byte)),
                    byte,
                    "{source:?} at {byte}"
                );
                assert_eq!(
                    index.point_to_utf16(index.byte_to_point(byte)),
                    index.byte_to_utf16(byte)
                );
            }
        }
    }

    #[test]
    fn columns_clamp_to_line_content_not_the_document() {
        let index = TextIndex::new("a😀\r\n漢\n");
        assert_eq!(index.utf16_to_byte(Position::new(0, u32::MAX)), 5);
        assert_eq!(index.utf16_to_byte(Position::new(1, u32::MAX)), 10);
        assert_eq!(index.utf16_to_byte(Position::new(2, u32::MAX)), 11);
        assert_eq!(index.utf16_to_byte(Position::new(u32::MAX, 0)), 11);
        assert_eq!(index.utf16_to_byte(Position::new(0, 2)), 1); // inside surrogate pair
        assert_eq!(
            index.point_to_utf16(Position::new(0, 3)),
            Position::new(0, 1)
        );
        assert_eq!(
            index.point_to_utf16(Position::new(u32::MAX, u32::MAX)),
            Position::new(2, 0)
        );
        assert_eq!(index.line(2), Some(""));
    }
}

#[cfg(test)]
mod edit_tests {
    use super::*;
    use crate::utils::{apply_text_edit, apply_text_edit_checked, position_to_byte};

    #[test]
    fn oversized_first_line_columns_do_not_panic_or_erase_following_lines() {
        let mut source = "abc\nxyz".to_owned();
        let end = apply_text_edit(
            &mut source,
            Position::new(0, 0),
            Position::new(0, u32::MAX),
            "",
        );
        assert_eq!(source, "\nxyz");
        assert_eq!(end, Position::new(0, 0));
        let mut source = "abc\nxyz".to_owned();
        apply_text_edit_checked(&mut source, Position::new(0, 300), Position::new(1, 0), "")
            .unwrap();
        assert_eq!(source, "abcxyz");
    }

    #[test]
    fn reversed_ranges_leave_source_unchanged() {
        let mut source = "é😀\r\n漢".to_owned();
        let original = source.clone();
        assert!(apply_text_edit_checked(
            &mut source,
            Position::new(1, 0),
            Position::new(0, 0),
            "BAD"
        )
        .is_none());
        assert_eq!(source, original);
        assert!(apply_text_edit_checked(
            &mut source,
            Position::new(0, 3),
            Position::new(0, 1),
            "BAD"
        )
        .is_none());
        assert_eq!(source, original);
    }

    #[test]
    fn edits_report_actual_byte_points_and_utf16_end_positions() {
        let mut source = "é😀z\r\n漢".to_owned();
        let edit = apply_text_edit_checked(
            &mut source,
            Position::new(0, 3),
            Position::new(0, 4),
            "漢\r\n😀\n",
        )
        .unwrap();
        assert_eq!(edit.start_byte, 6);
        assert_eq!(edit.old_end_byte, 7);
        assert_eq!(edit.start_position, Position::new(0, 6));
        assert_eq!(edit.old_end_position, Position::new(0, 7));
        assert_eq!(edit.new_end_position, Position::new(2, 0));
        assert_eq!(source, "é😀漢\r\n😀\n\r\n漢");
        let mut source = "é😀".to_owned();
        let end = apply_text_edit(&mut source, Position::new(0, 3), Position::new(0, 3), "漢");
        assert_eq!(end, Position::new(0, 4));
        let end = apply_text_edit(
            &mut source,
            Position::new(99, 999),
            Position::new(99, 999),
            "😀\n",
        );
        assert_eq!(end, Position::new(1, 0));
    }

    #[test]
    fn streaming_and_indexed_input_conversion_agree() {
        for source in ["", "é漢😀\r\nz\n", "a\n\n", "😀", "a\r\n"] {
            let index = TextIndex::new(source);
            for line in 0..5 {
                for column in [0, 1, 2, 3, 4, 5, 100, u32::MAX] {
                    let position = Position::new(line, column);
                    assert_eq!(
                        position_to_byte(source, position),
                        index.utf16_to_byte(position)
                    );
                }
            }
        }
    }
}
