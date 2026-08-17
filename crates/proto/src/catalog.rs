//! Protocol-agnostic locomotive catalogues (live roster, static list, address-only).

use crate::caps::LocoSource;
use crate::command::LocoId;
use crate::model::RosterEntry;
use crate::persist::StaticRosterEntry;

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
    /// Today's heuristic: live roster if non-empty, else static, else address-only.
    ///
    /// Preference + connect-time fallback (ARCHITECTURE.md §3.4) replaces this.
    #[must_use]
    pub fn prefer_live(live: &'a [RosterEntry], static_roster: &'a [StaticRosterEntry]) -> Self {
        if !live.is_empty() {
            Self::Server(ServerCatalog::new(live))
        } else if !static_roster.is_empty() {
            Self::Static(StaticCatalog::new(static_roster))
        } else {
            Self::Address(AddressCatalog)
        }
    }
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
    fn prefer_live_over_static() {
        let live = [live("A", 3, 'S')];
        let st = [stat("L1", "Pacific")];
        let c = Catalog::prefer_live(&live, &st);
        assert_eq!(c.source(), LocoSource::ServerRoster);
        assert_eq!(c.len(), 1);
        let e = c.entry(0).unwrap();
        assert_eq!(e.name, "A");
        assert_eq!(e.addr.as_str(), "S3");
    }

    #[test]
    fn prefer_static_when_live_empty() {
        let st = [stat("L1234", ""), stat("S99", "Switch")];
        let c = Catalog::prefer_live(&[], &st);
        assert_eq!(c.source(), LocoSource::StaticRoster);
        assert_eq!(c.entry(0).unwrap().name, "L1234");
        assert_eq!(c.entry(1).unwrap().name, "Switch");
        assert_eq!(c.entry(1).unwrap().addr.as_str(), "S99");
    }

    #[test]
    fn both_empty_is_address_only() {
        let c = Catalog::prefer_live(&[], &[]);
        assert_eq!(c.source(), LocoSource::AddressOnly);
        assert!(!c.allows_pick());
        assert!(c.entry(0).is_none());
    }
}
