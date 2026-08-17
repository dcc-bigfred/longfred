//! Protocol-agnostic locomotive catalogues (live roster, static list, address-only).

use crate::caps::{LocoSource, LocoSourceMask, ProtocolCaps};
use crate::command::LocoId;
use crate::model::RosterEntry;
use crate::persist::StaticRosterEntry;

/// Wait this long after connect for a live roster burst before treating
/// [`LocoSource::ServerRoster`] as unavailable (ARCHITECTURE.md §7).
pub const ROSTER_BURST_TIMEOUT_MS: u64 = 3_000;

/// One row in a [`LocoCatalog`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocoRef<'a> {
    pub name: &'a str,
    pub addr: heapless::String<8>,
}

/// Read-only view of locomotives the UI can pick from.
pub trait LocoCatalog {
    fn len(&self) -> usize;
    fn entry(&self, i: usize) -> Option<LocoRef<'_>>;
    fn allows_pick(&self) -> bool;
    fn source(&self) -> LocoSource;
}

/// Live roster from the command station.
#[derive(Clone, Copy, Debug)]
pub struct ServerCatalog<'a> {
    entries: &'a [RosterEntry],
}

impl<'a> ServerCatalog<'a> {
    #[must_use]
    pub fn new(entries: &'a [RosterEntry]) -> Self {
        Self { entries }
    }
}

impl LocoCatalog for ServerCatalog<'_> {
    fn len(&self) -> usize {
        self.entries.len()
    }

    fn entry(&self, i: usize) -> Option<LocoRef<'_>> {
        let e = self.entries.get(i)?;
        Some(LocoRef {
            name: e.name.as_str(),
            addr: roster_wire_addr(e),
        })
    }

    fn allows_pick(&self) -> bool {
        !self.entries.is_empty()
    }

    fn source(&self) -> LocoSource {
        LocoSource::ServerRoster
    }
}

/// `persist.static_roster` — shared by every protocol.
#[derive(Clone, Copy, Debug)]
pub struct StaticCatalog<'a> {
    entries: &'a [StaticRosterEntry],
}

impl<'a> StaticCatalog<'a> {
    #[must_use]
    pub fn new(entries: &'a [StaticRosterEntry]) -> Self {
        Self { entries }
    }
}

impl LocoCatalog for StaticCatalog<'_> {
    fn len(&self) -> usize {
        self.entries.len()
    }

    fn entry(&self, i: usize) -> Option<LocoRef<'_>> {
        let e = self.entries.get(i)?;
        Some(LocoRef {
            name: e.display_name(),
            addr: e.addr.clone(),
        })
    }

    fn allows_pick(&self) -> bool {
        !self.entries.is_empty()
    }

    fn source(&self) -> LocoSource {
        LocoSource::StaticRoster
    }
}

/// Manual DCC address; no list. Shared by every protocol.
#[derive(Clone, Copy, Debug, Default)]
pub struct AddressCatalog;

impl LocoCatalog for AddressCatalog {
    fn len(&self) -> usize {
        0
    }

    fn entry(&self, _i: usize) -> Option<LocoRef<'_>> {
        None
    }

    fn allows_pick(&self) -> bool {
        false
    }

    fn source(&self) -> LocoSource {
        LocoSource::AddressOnly
    }
}

/// Enum dispatch so UI can hold one catalogue without `dyn`.
#[derive(Clone, Copy, Debug)]
pub enum Catalog<'a> {
    Server(ServerCatalog<'a>),
    Static(StaticCatalog<'a>),
    Address(AddressCatalog),
}

impl<'a> Catalog<'a> {
    /// Catalogue for an already-resolved effective source.
    #[must_use]
    pub fn for_source(
        source: LocoSource,
        live: &'a [RosterEntry],
        static_roster: &'a [StaticRosterEntry],
    ) -> Self {
        match source {
            LocoSource::ServerRoster => Self::Server(ServerCatalog::new(live)),
            LocoSource::StaticRoster => Self::Static(StaticCatalog::new(static_roster)),
            LocoSource::AddressOnly => Self::Address(AddressCatalog),
        }
    }
}

/// Whether `src` currently has data (ARCHITECTURE.md §7 availability).
#[must_use]
pub fn source_available(src: LocoSource, live_len: usize, static_len: usize) -> bool {
    match src {
        LocoSource::ServerRoster => live_len > 0,
        LocoSource::StaticRoster => static_len > 0,
        LocoSource::AddressOnly => true,
    }
}

/// Effective source for this session. Fallback is one step to [`LocoSource::AddressOnly`].
///
/// `live_settled` is true after the first roster burst or
/// [`ROSTER_BURST_TIMEOUT_MS`], and immediately when `supported` cannot honour
/// a live roster. Until then a [`LocoSource::ServerRoster`] preference stays
/// preferred even if the live list is still empty.
#[must_use]
pub fn resolve_effective(
    pref: LocoSource,
    supported: LocoSourceMask,
    live_len: usize,
    static_len: usize,
    live_settled: bool,
) -> LocoSource {
    let waiting_for_burst =
        pref == LocoSource::ServerRoster && supported.contains(pref) && !live_settled;
    if waiting_for_burst {
        return pref;
    }
    if supported.contains(pref) && source_available(pref, live_len, static_len) {
        pref
    } else {
        LocoSource::AddressOnly
    }
}

/// Convenience: [`resolve_effective`] from a protocol's caps.
#[must_use]
pub fn resolve_effective_caps(
    pref: LocoSource,
    caps: ProtocolCaps,
    live_len: usize,
    static_len: usize,
    live_settled: bool,
) -> LocoSource {
    resolve_effective(pref, caps.loco_sources, live_len, static_len, live_settled)
}

/// Next or previous index in a catalogue. `None` when the list is empty.
///
/// Out-of-range `current` is treated as unknown (same as `None`).
#[must_use]
pub fn neighbour_index(len: usize, current: Option<usize>, next: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let current = current.filter(|&i| i < len);
    Some(match (current, next) {
        (Some(i), true) => (i + 1) % len,
        (Some(i), false) => (i + len - 1) % len,
        (None, true) => 0,
        (None, false) => len - 1,
    })
}

impl LocoCatalog for Catalog<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Server(c) => c.len(),
            Self::Static(c) => c.len(),
            Self::Address(c) => c.len(),
        }
    }

    fn entry(&self, i: usize) -> Option<LocoRef<'_>> {
        match self {
            Self::Server(c) => c.entry(i),
            Self::Static(c) => c.entry(i),
            Self::Address(c) => c.entry(i),
        }
    }

    fn allows_pick(&self) -> bool {
        match self {
            Self::Server(c) => c.allows_pick(),
            Self::Static(c) => c.allows_pick(),
            Self::Address(c) => c.allows_pick(),
        }
    }

    fn source(&self) -> LocoSource {
        match self {
            Self::Server(c) => c.source(),
            Self::Static(c) => c.source(),
            Self::Address(c) => c.source(),
        }
    }
}

fn roster_wire_addr(e: &RosterEntry) -> heapless::String<8> {
    let addr = u16::try_from(e.address.max(0)).unwrap_or(0);
    let long = matches!(e.length, 'L' | 'l') || addr >= 128;
    LocoId { addr, long }.to_wire()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ShortText;

    fn live(name: &str, address: i32, length: char) -> RosterEntry {
        let mut n = ShortText::new();
        let _ = n.push_str(name);
        RosterEntry {
            name: n,
            address,
            length,
        }
    }

    fn stat(addr: &str, name: &str) -> StaticRosterEntry {
        let mut e = StaticRosterEntry::default();
        let _ = e.addr.push_str(addr);
        let _ = e.name.push_str(name);
        e
    }

    #[test]
    fn for_source_picks_the_named_catalogue() {
        let live = [live("A", 3, 'S')];
        let st = [stat("L1", "Pacific")];
        let c = Catalog::for_source(LocoSource::ServerRoster, &live, &st);
        assert_eq!(c.source(), LocoSource::ServerRoster);
        assert_eq!(c.entry(0).unwrap().addr.as_str(), "S3");
        let c = Catalog::for_source(LocoSource::StaticRoster, &live, &st);
        assert_eq!(c.entry(0).unwrap().name, "Pacific");
        let c = Catalog::for_source(LocoSource::AddressOnly, &live, &st);
        assert!(!c.allows_pick());
    }

    #[test]
    fn wit_auto_waits_then_honours_live_roster() {
        let caps = crate::command::Protocol::WiThrottle.caps();
        let pref = LocoSource::ServerRoster;
        assert_eq!(
            resolve_effective_caps(pref, caps, 0, 1, false),
            LocoSource::ServerRoster
        );
        assert_eq!(
            resolve_effective_caps(pref, caps, 1, 1, true),
            LocoSource::ServerRoster
        );
        assert_eq!(
            resolve_effective_caps(pref, caps, 0, 1, true),
            LocoSource::AddressOnly
        );
    }

    #[test]
    fn fallback_is_address_only_not_static() {
        let caps = crate::command::Protocol::WiThrottle.caps();
        assert_eq!(
            resolve_effective_caps(LocoSource::ServerRoster, caps, 0, 1, true),
            LocoSource::AddressOnly
        );
        assert_eq!(
            resolve_effective_caps(LocoSource::StaticRoster, caps, 5, 0, true),
            LocoSource::AddressOnly
        );
    }

    #[test]
    fn z21_auto_falls_back_immediately() {
        let caps = crate::command::Protocol::Z21.caps();
        assert_eq!(
            resolve_effective_caps(LocoSource::ServerRoster, caps, 0, 2, false),
            LocoSource::AddressOnly
        );
        assert_eq!(
            resolve_effective_caps(LocoSource::StaticRoster, caps, 0, 2, true),
            LocoSource::StaticRoster
        );
    }

    #[test]
    fn disconnected_mask_cannot_honour_server_roster() {
        assert_eq!(
            resolve_effective(LocoSource::ServerRoster, LocoSourceMask::SHARED, 0, 0, true),
            LocoSource::AddressOnly
        );
    }

    #[test]
    fn neighbour_index_wraps_and_starts_from_ends() {
        assert_eq!(neighbour_index(0, None, true), None);
        assert_eq!(neighbour_index(3, None, true), Some(0));
        assert_eq!(neighbour_index(3, None, false), Some(2));
        assert_eq!(neighbour_index(3, Some(0), true), Some(1));
        assert_eq!(neighbour_index(3, Some(2), true), Some(0));
        assert_eq!(neighbour_index(3, Some(0), false), Some(2));
        assert_eq!(neighbour_index(3, Some(99), true), Some(0));
    }
}
