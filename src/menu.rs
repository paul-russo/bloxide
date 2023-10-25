#[derive(Copy, Clone, Debug)]
pub struct MenuItem<'a> {
    pub label: &'a str,
    pub id: &'a str,
}

#[derive(Debug)]
pub struct Menu<'a> {
    pub title: &'a str,
    pub items: Vec<MenuItem<'a>>,
    pub active_index: usize,
    pub is_visible: bool,
}

#[derive(Copy, Clone, Debug)]
pub struct MenuInput {
    pub up: bool,
    pub down: bool,
    pub select: bool,
}

impl<'a> Menu<'a> {
    pub fn new(title: &'a str, items: Vec<MenuItem<'a>>) -> Self {
        Self {
            title,
            items,
            active_index: 0,
            is_visible: false,
        }
    }

    /// Update the menu state with the given input, and possibly return the selected item if input.select is true.
    pub fn update(&mut self, input: MenuInput) -> Option<&str> {
        if !self.is_visible {
            return None;
        }

        let mut next_index = self.active_index as isize;

        if input.select {
            return Some(self.items[self.active_index].id);
        }

        if input.up {
            next_index -= 1;
        }

        if input.down {
            next_index += 1;
        }

        self.active_index = next_index.rem_euclid(self.items.len() as isize) as usize;

        None
    }
}
