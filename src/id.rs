/// Stable identifier used to match layout elements across frames.
///
/// The `hash` is deterministic for the same label, offset and base. The
/// original label is kept so callers can still query results by string id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ElementId {
    /// Deterministic hash used for compact identity comparisons.
    pub hash: u32,
    /// Optional index mixed into the hash.
    pub offset: u32,
    /// Optional base hash for locally scoped ids.
    pub base: u32,
    /// Human-readable id label.
    pub label: String,
}

impl ElementId {
    /// Creates an id from a label.
    pub fn new(label: impl Into<String>) -> Self {
        let label = label.into();
        Self::with_offset_and_base(label, 0, 0)
    }

    /// Creates a repeated id by mixing `offset` into the label hash.
    pub fn indexed(label: impl Into<String>, offset: u32) -> Self {
        let label = label.into();
        Self::with_offset_and_base(label, offset, 0)
    }

    /// Creates an id scoped under `base`.
    pub fn local(label: impl Into<String>, base: u32) -> Self {
        let label = label.into();
        Self::with_offset_and_base(label, 0, base)
    }

    /// Creates an id from all hashing components.
    pub fn with_offset_and_base(label: impl Into<String>, offset: u32, base: u32) -> Self {
        let label = label.into();
        let hash = hash_id(&label, offset, base);
        Self {
            hash,
            offset,
            base,
            label,
        }
    }
}

fn hash_id(label: &str, offset: u32, base: u32) -> u32 {
    let mut hash = 0x811c_9dc5_u32 ^ base;
    for byte in label.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    hash ^ offset
}
