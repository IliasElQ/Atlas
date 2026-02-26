use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

// ── Actions ────────────────────────────────────────────────────────

/// Mapped action from a key event
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Quit,
    MoveUp,
    MoveDown,
    Enter,
    Back,
    Refresh,
    NextPage,
    PrevPage,
    ToggleLogs,
    Rerun,
    Cancel,
    OpenInBrowser,
    Search,
    None,
}

/// Map key events to app actions
pub fn map_key_to_action(key: KeyEvent) -> Action {
    // Ctrl+C always quits
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Action::Quit;
    }

    match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Up | KeyCode::Char('k') => Action::MoveUp,
        KeyCode::Down | KeyCode::Char('j') => Action::MoveDown,
        KeyCode::Enter | KeyCode::Char('l') => Action::Enter,
        KeyCode::Esc | KeyCode::Char('h') | KeyCode::Backspace => Action::Back,
        KeyCode::Char('r') => Action::Refresh,
        KeyCode::Char('n') | KeyCode::Right => Action::NextPage,
        KeyCode::Char('p') | KeyCode::Left => Action::PrevPage,
        KeyCode::Char('L') => Action::ToggleLogs,
        KeyCode::Char('R') => Action::Rerun,
        KeyCode::Char('C') => Action::Cancel,
        KeyCode::Char('o') => Action::OpenInBrowser,
        KeyCode::Char('/') => Action::Search,
        _ => Action::None,
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn key_with_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn test_quit_actions() {
        assert_eq!(map_key_to_action(key(KeyCode::Char('q'))), Action::Quit);
        assert_eq!(
            map_key_to_action(key_with_mod(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Action::Quit
        );
    }

    #[test]
    fn test_navigation_actions() {
        assert_eq!(map_key_to_action(key(KeyCode::Up)), Action::MoveUp);
        assert_eq!(map_key_to_action(key(KeyCode::Char('k'))), Action::MoveUp);
        assert_eq!(map_key_to_action(key(KeyCode::Down)), Action::MoveDown);
        assert_eq!(map_key_to_action(key(KeyCode::Char('j'))), Action::MoveDown);
        assert_eq!(map_key_to_action(key(KeyCode::Enter)), Action::Enter);
        assert_eq!(map_key_to_action(key(KeyCode::Char('l'))), Action::Enter);
        assert_eq!(map_key_to_action(key(KeyCode::Esc)), Action::Back);
        assert_eq!(map_key_to_action(key(KeyCode::Char('h'))), Action::Back);
    }

    #[test]
    fn test_action_keys() {
        assert_eq!(map_key_to_action(key(KeyCode::Char('r'))), Action::Refresh);
        assert_eq!(map_key_to_action(key(KeyCode::Char('R'))), Action::Rerun);
        assert_eq!(map_key_to_action(key(KeyCode::Char('C'))), Action::Cancel);
        assert_eq!(
            map_key_to_action(key(KeyCode::Char('o'))),
            Action::OpenInBrowser
        );
    }

    #[test]
    fn test_pagination() {
        assert_eq!(map_key_to_action(key(KeyCode::Char('n'))), Action::NextPage);
        assert_eq!(map_key_to_action(key(KeyCode::Right)), Action::NextPage);
        assert_eq!(map_key_to_action(key(KeyCode::Char('p'))), Action::PrevPage);
        assert_eq!(map_key_to_action(key(KeyCode::Left)), Action::PrevPage);
    }

    #[test]
    fn test_unknown_key_returns_none() {
        assert_eq!(map_key_to_action(key(KeyCode::Char('z'))), Action::None);
        assert_eq!(map_key_to_action(key(KeyCode::F(1))), Action::None);
    }
}
