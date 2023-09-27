pub fn is_all_none<T>(options: &Vec<Option<T>>) -> bool {
    for option in options {
        if option.is_some() {
            return false;
        }
    }

    true
}

pub fn is_all_none_col<T>(rows: &Vec<Vec<Option<T>>>, col_id: usize) -> bool {
    if col_id > rows[0].len() - 1 {
        panic!(
            "Attempt to read column {} of row with length {}",
            col_id,
            rows[0].len()
        );
    }

    for row_id in 0..rows.len() {
        if rows[row_id][col_id].is_some() {
            return false;
        }
    }

    true
}

pub fn get_first_non_empty_row_id<T>(rows: &Vec<Vec<Option<T>>>) -> Option<usize> {
    for (row_id, row) in rows.iter().enumerate() {
        if !is_all_none(&row) {
            return Some(row_id);
        }
    }

    None
}
