use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy)]
pub(super) enum Action {
    Next,
    Prev,
    First,
    Last,
    Quit,
}

// `matches!` rather than a `match` keeps this off the non-exhaustive `KeyCode`
// enum's wildcard-match lint.
pub(super) fn action_for(key: &KeyEvent) -> Option<Action> {
    use KeyCode as Code;

    if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, Code::Char('c')) {
        return Some(Action::Quit);
    }
    if matches!(
        key.code,
        Code::Right | Code::Down | Code::PageDown | Code::Char(' ' | 'l' | 'j' | 'n')
    ) {
        return Some(Action::Next);
    }
    if matches!(
        key.code,
        Code::Left | Code::Up | Code::PageUp | Code::Backspace | Code::Char('h' | 'k' | 'p')
    ) {
        return Some(Action::Prev);
    }
    if matches!(key.code, Code::Home | Code::Char('g')) {
        return Some(Action::First);
    }
    if matches!(key.code, Code::End | Code::Char('G')) {
        return Some(Action::Last);
    }
    if matches!(key.code, Code::Char('q') | Code::Esc) {
        return Some(Action::Quit);
    }
    None
}
