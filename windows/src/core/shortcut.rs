//! A hold-to-zoom shortcut: modifiers, optionally with one ordinary key.
use super::modifiers::Modifiers;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shortcut {
    pub modifiers: Modifiers,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<u16>,
}

impl Shortcut {
    pub fn valid(self) -> bool {
        self.key.is_none_or(supported_key) && (!self.modifiers.is_empty() || self.key.is_some())
    }

    pub fn label(self) -> String {
        let mut parts: Vec<String> = self.modifiers.names().into_iter().map(str::to_owned).collect();
        if let Some(key) = self.key { parts.push(key_label(key)); }
        parts.join(" + ")
    }

    pub fn satisfied_by(self, held: Modifiers, keys: &[u64; 4]) -> bool {
        self.valid() && (self.modifiers.is_empty() || self.modifiers.satisfied_by(held)) && self.key.is_none_or(|key| keys[usize::from(key / 64)] & (1 << (key % 64)) != 0)
    }
}

pub fn supported_key(key: u16) -> bool {
    matches!(key, 0x08 | 0x0D | 0x20..=0x28 | 0x2D..=0x2E | 0x30..=0x39 | 0x41..=0x5A | 0x60..=0x6F | 0x70..=0x87 | 0xBA..=0xC0 | 0xDB..=0xDE)
}

pub fn key_label(key: u16) -> String {
    match key {
        0x30..=0x39 | 0x41..=0x5A => return char::from_u32(u32::from(key)).unwrap().to_string(),
        0x70..=0x87 => return format!("F{}", key - 0x6F),
        0x60..=0x69 => return format!("Num {}", key - 0x60),
        0x08 => "Backspace", 0x0D => "Enter", 0x20 => "Space", 0x21 => "Page Up", 0x22 => "Page Down",
        0x23 => "End", 0x24 => "Home", 0x25 => "Left", 0x26 => "Up", 0x27 => "Right", 0x28 => "Down",
        0x2D => "Insert", 0x2E => "Delete", 0x6A => "Num *", 0x6B => "Num +", 0x6C => "Num separator", 0x6D => "Num -", 0x6E => "Num .", 0x6F => "Num /",
        0xBA => ";", 0xBB => "=", 0xBC => ",", 0xBD => "-", 0xBE => ".", 0xBF => "/", 0xC0 => "`", 0xDB => "[", 0xDC => "\\", 0xDD => "]", 0xDE => "'",
        _ => return format!("Key {key}"),
    }.into()
}

/// Collect until any key in the combination is released. No file writes while recording.
#[derive(Default)]
pub struct Recorder {
    draft: Shortcut,
    waiting: bool,
}
impl Recorder {
    pub fn waiting_for_release() -> Self { Self {waiting: true, ..Self::default()} }
    pub fn draft(&self) -> Shortcut { self.draft }
    pub fn sample(&mut self, held: Shortcut) -> Option<Shortcut> {
        if self.waiting {
            if !held.valid() { self.waiting = false; }
            return None;
        }
        if self.draft.valid() && (!held.modifiers.contains(self.draft.modifiers) || self.draft.key.is_some_and(|k| held.key != Some(k))) {
            return Some(self.draft);
        }
        if held.valid() { self.draft = held; }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn records_modifier_only_and_key_combinations_on_release() {
        let mut r=Recorder::default();
        assert_eq!(r.sample(Shortcut{modifiers:Modifiers::CTRL,key:None}),None);
        let combo=Shortcut{modifiers:Modifiers::CTRL | Modifiers::ALT,key:Some(0x4B)};
        assert_eq!(r.sample(combo),None);
        assert_eq!(r.sample(Shortcut{key:None,..combo}),Some(combo));
        assert_eq!(combo.label(),"Ctrl + Alt + K");
        let mut r=Recorder::default();
        let shift=Shortcut{modifiers:Modifiers::SHIFT,key:None};
        assert_eq!(r.sample(shift),None);
        assert_eq!(r.sample(Shortcut::default()),Some(shift));
    }
    #[test]
    fn ignores_activation_key_until_it_is_released() {
        let mut r=Recorder::waiting_for_release();
        let enter=Shortcut{modifiers:Modifiers::NONE,key:Some(0x0D)};
        assert_eq!(r.sample(enter),None);
        assert_eq!(r.sample(Shortcut::default()),None);
        let shift=Shortcut{modifiers:Modifiers::SHIFT,key:None};
        assert_eq!(r.sample(shift),None);
        assert_eq!(r.sample(Shortcut::default()),Some(shift));
    }

    #[test]
    fn regular_key_is_required_and_invalid_keys_cannot_index_out_of_bounds() {
        let s=Shortcut{modifiers:Modifiers::CTRL,key:Some(0x4B)};
        let mut keys=[0;4];
        assert!(!s.satisfied_by(Modifiers::CTRL,&keys));
        keys[1]=1 << (0x4B % 64);
        assert!(s.satisfied_by(Modifiers::CTRL,&keys));
        assert!(!s.satisfied_by(Modifiers::ALT,&keys));
        assert!(!Shortcut{key:Some(999),..s}.satisfied_by(Modifiers::ALL,&keys));
        assert!(!Shortcut::default().satisfied_by(Modifiers::ALL,&keys));
    }
}
