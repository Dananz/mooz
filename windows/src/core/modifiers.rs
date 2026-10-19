//! The trigger modifier combo, persisted as a small bit set. Windows has no
//! Command key, so Win stands in for it.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Modifiers(pub u8);

impl Modifiers {
    pub const NONE: Self = Self(0);
    pub const CTRL: Self = Self(1);
    pub const ALT: Self = Self(2);
    pub const SHIFT: Self = Self(4);
    pub const WIN: Self = Self(8);
    pub const ALL: Self = Self(15);

    /// Display order, matching how Windows writes shortcuts (Ctrl + Alt + Shift + Win).
    pub const ORDERED: [(Self, &'static str); 4] = [
        (Self::CTRL, "Ctrl"),
        (Self::ALT, "Alt"),
        (Self::SHIFT, "Shift"),
        (Self::WIN, "Win"),
    ];

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn is_empty(self) -> bool {
        self.0 & Self::ALL.0 == 0
    }

    pub fn masked(self) -> Self {
        Self(self.0 & Self::ALL.0)
    }

    pub fn names(self) -> Vec<&'static str> {
        Self::ORDERED
            .iter()
            .filter(|(m, _)| self.contains(*m))
            .map(|(_, n)| *n)
            .collect()
    }

    /// "Ctrl + Shift", or "None" when empty.
    pub fn label(self) -> String {
        let names = self.names();
        if names.is_empty() {
            "None".into()
        } else {
            names.join(" + ")
        }
    }

    /// True while every required modifier is held. Extra modifiers are allowed
    /// (subset match) so brushing another key mid-gesture doesn't drop it, the
    /// same rule as the macOS interceptor. An empty combo never matches.
    pub fn satisfied_by(self, held: Self) -> bool {
        let required = self.masked();
        !required.is_empty() && held.contains(required)
    }
}

/// What the low-level keyboard hook last saw for one physical key (one side
/// of a modifier), counting only input Mooz didn't inject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeySide {
    /// No event seen yet (e.g. the key was already down when Mooz started).
    Unknown,
    Down,
    Up,
}

/// Whether a modifier counts as held. The OS async state alone can't be
/// trusted: Mooz re-presses the trigger keys after each zoom step, so if the
/// user let go in that instant the OS still reports the key down. The hook's
/// physical view decides; async-only is the fallback when the hook has seen
/// nothing for either side.
pub fn physically_held(async_down: bool, left: KeySide, right: KeySide) -> bool {
    use KeySide::*;
    async_down && matches!((left, right), (Down, _) | (_, Down) | (Unknown, Unknown))
}

/// One side of a modifier as the emitter sees it before a zoom step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SideAction {
    /// Up: nothing to do.
    Leave,
    /// Held by the user: lift around the step and press back afterwards.
    LiftAndRestore,
    /// Down only because of an earlier re-press: release it, don't restore.
    Release,
}

pub fn side_action(async_down: bool, side: KeySide) -> SideAction {
    match (async_down, side) {
        (false, _) => SideAction::Leave,
        (true, KeySide::Up) => SideAction::Release,
        (true, _) => SideAction::LiftAndRestore,
    }
}

impl std::ops::BitOr for Modifiers {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subset_match_allows_extra_modifiers() {
        let required = Modifiers::SHIFT;
        assert!(required.satisfied_by(Modifiers::SHIFT));
        assert!(required.satisfied_by(Modifiers::SHIFT | Modifiers::CTRL));
        assert!(!required.satisfied_by(Modifiers::CTRL));
        assert!(!required.satisfied_by(Modifiers::NONE));
    }

    #[test]
    fn combo_requires_every_key() {
        let required = Modifiers::CTRL | Modifiers::SHIFT;
        assert!(!required.satisfied_by(Modifiers::SHIFT));
        assert!(required.satisfied_by(Modifiers::CTRL | Modifiers::SHIFT));
    }

    #[test]
    fn empty_combo_never_matches() {
        assert!(!Modifiers::NONE.satisfied_by(Modifiers::ALL));
    }

    #[test]
    fn held_falls_back_to_async_when_hook_saw_nothing() {
        use KeySide::*;
        assert!(physically_held(true, Unknown, Unknown));
        assert!(!physically_held(false, Unknown, Unknown));
    }

    #[test]
    fn held_when_either_side_is_physically_down() {
        use KeySide::*;
        assert!(physically_held(true, Down, Unknown));
        assert!(physically_held(true, Up, Down));
    }

    #[test]
    fn released_side_overrides_a_stale_async_down() {
        // The late re-press case: OS says down, the user already let go.
        use KeySide::*;
        assert!(!physically_held(true, Up, Unknown));
        assert!(!physically_held(true, Up, Up));
    }

    #[test]
    fn stuck_side_is_released_not_restored() {
        assert_eq!(side_action(true, KeySide::Up), SideAction::Release);
        assert_eq!(side_action(true, KeySide::Down), SideAction::LiftAndRestore);
        assert_eq!(side_action(true, KeySide::Unknown), SideAction::LiftAndRestore);
        assert_eq!(side_action(false, KeySide::Up), SideAction::Leave);
    }

    #[test]
    fn label_uses_windows_order() {
        assert_eq!((Modifiers::WIN | Modifiers::CTRL).label(), "Ctrl + Win");
        assert_eq!(Modifiers::NONE.label(), "None");
    }
}
