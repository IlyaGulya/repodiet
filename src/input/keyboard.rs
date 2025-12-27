use crossterm::event::{KeyCode, KeyEvent};

/// Map a character to its QWERTY equivalent for layout-independent key handling.
/// This allows vim-style navigation to work regardless of keyboard layout.
pub fn to_qwerty(c: char) -> char {
    match c {
        // Russian ЙЦУКЕН layout -> QWERTY
        'й' => 'q', 'Й' => 'Q',
        'ц' => 'w', 'Ц' => 'W',
        'у' => 'e', 'У' => 'E',
        'к' => 'r', 'К' => 'R',
        'е' => 't', 'Е' => 'T',
        'н' => 'y', 'Н' => 'Y',
        'г' => 'u', 'Г' => 'U',
        'ш' => 'i', 'Ш' => 'I',
        'щ' => 'o', 'Щ' => 'O',
        'з' => 'p', 'З' => 'P',
        'ф' => 'a', 'Ф' => 'A',
        'ы' => 's', 'Ы' => 'S',
        'в' => 'd', 'В' => 'D',
        'а' => 'f', 'А' => 'F',
        'п' => 'g', 'П' => 'G',
        'р' => 'h', 'Р' => 'H',
        'о' => 'j', 'О' => 'J',
        'л' => 'k', 'Л' => 'K',
        'д' => 'l', 'Д' => 'L',
        'я' => 'z', 'Я' => 'Z',
        'ч' => 'x', 'Ч' => 'X',
        'с' => 'c', 'С' => 'C',
        'м' => 'v', 'М' => 'V',
        'и' => 'b', 'И' => 'B',
        'т' => 'n', 'Т' => 'N',
        'ь' => 'm', 'Ь' => 'M',
        // Pass through unchanged if not mapped
        _ => c,
    }
}

/// Check if a KeyCode matches the expected character, accounting for keyboard layouts.
/// When expected is uppercase, match is case-sensitive.
/// When expected is lowercase, match is case-insensitive.
pub fn key_matches(key: &KeyCode, expected: char) -> bool {
    match key {
        KeyCode::Char(c) => {
            let normalized = to_qwerty(*c);
            if expected.is_uppercase() {
                // Case-sensitive match for uppercase expected (e.g., 'L' for Large Blobs)
                normalized == expected
            } else {
                // Case-insensitive match for lowercase expected (e.g., 'l' for enter/right)
                normalized == expected || normalized.to_ascii_lowercase() == expected
            }
        }
        _ => false,
    }
}

/// User intents derived from keyboard input
#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    // Navigation
    MoveUp,
    MoveDown,
    Enter,
    Back,

    // Mode switching
    ShowTree,
    ShowExtensions,
    ShowLargeBlobs,
    EnterSearch,

    // Filters
    ToggleDeletedOnly,

    // Actions
    Quit,

    // Search input
    SearchChar(char),
    SearchBackspace,
}

/// View modes for mapping keys to intents
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    Tree,
    ByExtension,
    LargeBlobs,
}

/// Map a key event to a user intent based on the current view mode and search state
pub fn map_key_to_intent(key: &KeyEvent, mode: ViewMode, search_mode: bool, is_at_root: bool) -> Option<Intent> {
    let code = &key.code;

    if search_mode {
        return match code {
            KeyCode::Esc => Some(Intent::ShowTree),  // Exit search
            KeyCode::Enter => Some(Intent::Enter),
            KeyCode::Up => Some(Intent::MoveUp),
            KeyCode::Down => Some(Intent::MoveDown),
            KeyCode::Backspace => Some(Intent::SearchBackspace),
            KeyCode::Char(c) => Some(Intent::SearchChar(*c)),
            _ => None,
        };
    }

    // Common keys across all modes
    if key_matches(code, 'q') {
        return Some(Intent::Quit);
    }
    if *code == KeyCode::Char('/') {
        return Some(Intent::EnterSearch);
    }

    match mode {
        ViewMode::Tree => {
            if *code == KeyCode::Esc {
                if is_at_root {
                    return Some(Intent::Quit);
                } else {
                    return Some(Intent::Back);
                }
            }
            if *code == KeyCode::Up || key_matches(code, 'k') {
                return Some(Intent::MoveUp);
            }
            if *code == KeyCode::Down || key_matches(code, 'j') {
                return Some(Intent::MoveDown);
            }
            if *code == KeyCode::Enter || *code == KeyCode::Right || key_matches(code, 'l') {
                return Some(Intent::Enter);
            }
            if *code == KeyCode::Backspace || *code == KeyCode::Left || key_matches(code, 'h') {
                return Some(Intent::Back);
            }
            if key_matches(code, 'd') {
                return Some(Intent::ToggleDeletedOnly);
            }
            if key_matches(code, 't') {
                return Some(Intent::ShowExtensions);
            }
            if key_matches(code, 'L') {
                return Some(Intent::ShowLargeBlobs);
            }
            None
        }
        ViewMode::ByExtension => {
            if *code == KeyCode::Esc || key_matches(code, 't') {
                return Some(Intent::ShowTree);
            }
            if *code == KeyCode::Up || key_matches(code, 'k') {
                return Some(Intent::MoveUp);
            }
            if *code == KeyCode::Down || key_matches(code, 'j') {
                return Some(Intent::MoveDown);
            }
            if key_matches(code, 'L') {
                return Some(Intent::ShowLargeBlobs);
            }
            None
        }
        ViewMode::LargeBlobs => {
            if *code == KeyCode::Esc || key_matches(code, 'l') || key_matches(code, 'L') {
                return Some(Intent::ShowTree);
            }
            if *code == KeyCode::Up || key_matches(code, 'k') {
                return Some(Intent::MoveUp);
            }
            if *code == KeyCode::Down || key_matches(code, 'j') {
                return Some(Intent::MoveDown);
            }
            if *code == KeyCode::Enter {
                return Some(Intent::Enter);
            }
            None
        }
    }
}
