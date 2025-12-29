/// Move selection up with wraparound
pub fn move_up(selected: &mut usize, len: usize) {
    if len == 0 {
        *selected = 0;
        return;
    }
    *selected = if *selected == 0 { len - 1 } else { *selected - 1 };
}

/// Move selection down with wraparound
pub fn move_down(selected: &mut usize, len: usize) {
    if len == 0 {
        *selected = 0;
        return;
    }
    *selected = if *selected + 1 >= len { 0 } else { *selected + 1 };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_move_up_wraparound() {
        let mut selected = 0;
        move_up(&mut selected, 3);
        assert_eq!(selected, 2);
    }

    #[test]
    fn test_move_up_normal() {
        let mut selected = 2;
        move_up(&mut selected, 3);
        assert_eq!(selected, 1);
    }

    #[test]
    fn test_move_down_wraparound() {
        let mut selected = 2;
        move_down(&mut selected, 3);
        assert_eq!(selected, 0);
    }

    #[test]
    fn test_move_down_normal() {
        let mut selected = 0;
        move_down(&mut selected, 3);
        assert_eq!(selected, 1);
    }

    #[test]
    fn test_empty_list() {
        let mut selected = 5;
        move_up(&mut selected, 0);
        assert_eq!(selected, 0);

        selected = 5;
        move_down(&mut selected, 0);
        assert_eq!(selected, 0);
    }
}
