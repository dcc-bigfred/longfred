//! Persistence (NVS): WiFi passwords, saved locos, device identity (one NVS sector).

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_sync::watch::Watch;
use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
use esp_bootloader_esp_idf::partitions::{
    DataPartitionSubType, PARTITION_TABLE_MAX_LEN, PartitionType, read_partition_table,
};
use esp_hal::rng::Rng;
use esp_storage::FlashStorage;
use heapless::String;
use log::{info, warn};
use longfred_proto::persist::{
    DeviceIdentity, Language, MAX_SAVED_LOCOS, MAX_WIFI_HOSTNAME_LEN, PersistRecord, RosterMode,
    SavedLoco, SavedServer, StaticIpConfig, id_from_entropy, wifi_hostname_from_entropy,
};

/// Latest NVS snapshot. `Watch` (not `Signal`) so domain **and** HTTP provisioning
/// can both see the same record; a `Signal` has a single waiter and the HTTP sync
/// task was consuming the boot snapshot before the UI could apply it.
pub static PERSIST_LOADED: Watch<CriticalSectionRawMutex, PersistRecord, 4> = Watch::new();

fn publish_persist(rec: &PersistRecord) {
    PERSIST_LOADED.sender().send(rec.clone());
}

/// Signalled after a storage write that requested acknowledgement
/// ([`StorageCmd::SetProgrammingMode`], [`StorageCmd::ReplaceRecord`]).
/// `true` = flash persist succeeded.
pub static STORAGE_ACK: Signal<CriticalSectionRawMutex, bool> = Signal::new();

pub enum StorageCmd {
    SavePassword {
        ssid: String<32>,
        password: String<64>,
    },
    SaveLocos(heapless::Vec<SavedLoco, MAX_SAVED_LOCOS>),
    SaveNetwork(StaticIpConfig),
    SaveDevice(DeviceIdentity),
    RegenerateDeviceId,
    SaveLanguage(Language),
    SaveRosterMode(RosterMode),
    SavePairingCode(String<6>),
    SaveServer(SavedServer),
    SetProgrammingMode(bool),
    ReplaceRecord(PersistRecord),
    Clear,
}

pub static STORAGE_CTRL: Channel<CriticalSectionRawMutex, StorageCmd, 4> = Channel::new();

/// Flash mutex shared by NVS persistence and HTTP OTA.
pub type SharedFlash = Mutex<CriticalSectionRawMutex, FlashStorage<'static>>;

/// Boot-time NVS snapshot used to choose STA vs programming path.
#[derive(Clone)]
pub struct BootState {
    pub wifi_hostname: heapless::String<MAX_WIFI_HOSTNAME_LEN>,
    pub programming_mode: bool,
    pub has_wifi_credentials: bool,
    pub record: PersistRecord,
}

const SECTOR: usize = 4096;

/// `read_partition_table` rejects a buffer longer than [`PARTITION_TABLE_MAX_LEN`]
/// (0xC00). A 4 KiB sector-sized buffer used to make every NVS load/store fail.
fn load(flash: &mut FlashStorage<'_>) -> Option<PersistRecord> {
    let mut pt_buf = [0u8; PARTITION_TABLE_MAX_LEN];
    let pt = match read_partition_table(flash, &mut pt_buf) {
        Ok(pt) => pt,
        Err(e) => {
            warn!("storage: partition table read failed: {e:?}");
            return None;
        }
    };
    let nvs = match pt.find_partition(PartitionType::Data(DataPartitionSubType::Nvs)) {
        Ok(Some(nvs)) => nvs,
        Ok(None) => {
            warn!("storage: nvs partition not found");
            return None;
        }
        Err(e) => {
            warn!("storage: nvs lookup failed: {e:?}");
            return None;
        }
    };
    let mut region = nvs.as_embedded_storage(flash);
    let mut sector = [0u8; SECTOR];
    if ReadNorFlash::read(&mut region, 0, &mut sector).is_err() {
        warn!("storage: nvs sector read failed");
        return None;
    }
    PersistRecord::decode(&sector)
}

fn persist(flash: &mut FlashStorage<'_>, rec: &PersistRecord) -> bool {
    let mut pt_buf = [0u8; PARTITION_TABLE_MAX_LEN];
    let pt = match read_partition_table(flash, &mut pt_buf) {
        Ok(pt) => pt,
        Err(e) => {
            warn!("storage: partition table read failed: {e:?}");
            return false;
        }
    };
    let nvs = match pt.find_partition(PartitionType::Data(DataPartitionSubType::Nvs)) {
        Ok(Some(nvs)) => nvs,
        Ok(None) => {
            warn!("storage: nvs partition not found");
            return false;
        }
        Err(e) => {
            warn!("storage: nvs lookup failed: {e:?}");
            return false;
        }
    };
    let mut region = nvs.as_embedded_storage(flash);
    let mut sector = [0xFFu8; SECTOR];
    if rec.encode(&mut sector).is_none() {
        warn!("storage: encode failed");
        return false;
    }
    // Trailing 0xFF is padding so the write length stays a multiple of the
    // flash word (4 B). `encode` returns a variable size that is usually not.
    if let Err(e) = region.erase(0, SECTOR as u32) {
        warn!("storage: erase failed: {e:?}");
        return false;
    }
    if let Err(e) = NorFlash::write(&mut region, 0, &sector) {
        warn!("storage: write failed: {e:?}");
        return false;
    }
    true
}

async fn persist_shared(flash: &SharedFlash, rec: &PersistRecord) -> bool {
    let mut g = flash.lock().await;
    persist(&mut g, rec)
}

/// Synchronous NVS write (boot path before the storage task runs).
pub fn write_record(flash: &mut FlashStorage<'_>, rec: &PersistRecord) {
    if !persist(flash, rec) {
        warn!("storage: write_record failed");
    }
}

fn ensure_device_id(rec: &mut PersistRecord, entropy: u32) {
    if rec.device.id == 0 {
        rec.device.id = id_from_entropy(entropy);
        if rec.device.name.is_empty() {
            rec.device.name.clear();
            let _ = rec.device.name.push_str(crate::config::DEFAULT_DEVICE_NAME);
        }
    }
}

fn ensure_wifi_hostname(rec: &mut PersistRecord, entropy: u32) {
    if rec.wifi_hostname.is_empty() {
        rec.wifi_hostname = wifi_hostname_from_entropy(entropy);
    }
}

fn regenerate_device_id(rec: &mut PersistRecord) {
    let rng = Rng::new();
    rec.device.id = id_from_entropy(rng.random());
}

/// Load NVS and ensure device id + DHCP hostname exist (called before embassy-net init).
pub fn ensure_boot(flash: &mut FlashStorage<'_>, boot_entropy: u32) -> BootState {
    let mut rec = load(flash).unwrap_or_default();
    let mut dirty = false;
    if rec.wifi_hostname.is_empty() {
        ensure_wifi_hostname(&mut rec, boot_entropy);
        dirty = true;
    }
    if rec.device.id == 0 {
        ensure_device_id(&mut rec, boot_entropy);
        dirty = true;
    }
    if dirty && !persist(flash, &rec) {
        warn!("storage: boot persist failed");
    }
    BootState {
        wifi_hostname: rec.wifi_hostname.clone(),
        programming_mode: rec.programming_mode,
        has_wifi_credentials: !rec.credentials.is_empty(),
        record: rec,
    }
}

/// Load NVS and ensure device id + DHCP hostname exist (called before embassy-net init).
pub fn ensure_boot_hostname(
    flash: &mut FlashStorage<'_>,
    boot_entropy: u32,
) -> heapless::String<MAX_WIFI_HOSTNAME_LEN> {
    ensure_boot(flash, boot_entropy).wifi_hostname
}

#[embassy_executor::task]
pub async fn task(flash: &'static SharedFlash, boot_entropy: u32) {
    let mut rec = {
        let mut g = flash.lock().await;
        load(&mut g).unwrap_or_default()
    };
    let mut dirty = false;
    if rec.device.id == 0 {
        ensure_device_id(&mut rec, boot_entropy);
        dirty = true;
    }
    if rec.wifi_hostname.is_empty() {
        ensure_wifi_hostname(&mut rec, boot_entropy);
        dirty = true;
    }
    if dirty && !persist_shared(flash, &rec).await {
        warn!("storage: initial persist failed");
    }
    info!("wifi hostname: {}", rec.wifi_hostname.as_str());
    publish_persist(&rec);

    let rx = STORAGE_CTRL.receiver();
    loop {
        match rx.receive().await {
            StorageCmd::SavePassword { ssid, password } => {
                rec.set_password(ssid.as_str(), password.as_str());
                let _ = persist_shared(flash, &rec).await;
                publish_persist(&rec);
            }
            StorageCmd::SaveLocos(locos) => {
                rec.locos = locos;
                let _ = persist_shared(flash, &rec).await;
                publish_persist(&rec);
            }
            StorageCmd::SaveNetwork(cfg) => {
                rec.network = Some(cfg);
                let _ = persist_shared(flash, &rec).await;
                publish_persist(&rec);
            }
            StorageCmd::SaveDevice(device) => {
                rec.device = device;
                let _ = persist_shared(flash, &rec).await;
                publish_persist(&rec);
            }
            StorageCmd::RegenerateDeviceId => {
                regenerate_device_id(&mut rec);
                let _ = persist_shared(flash, &rec).await;
                publish_persist(&rec);
            }
            StorageCmd::SaveLanguage(lang) => {
                rec.language = lang;
                rec.language_chosen = true;
                let _ = persist_shared(flash, &rec).await;
                publish_persist(&rec);
            }
            StorageCmd::SaveRosterMode(mode) => {
                rec.roster_mode = mode;
                let _ = persist_shared(flash, &rec).await;
                publish_persist(&rec);
            }
            StorageCmd::SavePairingCode(code) => {
                rec.bigfred_pairing_code = code;
                let _ = persist_shared(flash, &rec).await;
                publish_persist(&rec);
            }
            StorageCmd::SaveServer(server) => {
                rec.last_server = Some(server);
                let _ = persist_shared(flash, &rec).await;
                publish_persist(&rec);
            }
            StorageCmd::SetProgrammingMode(on) => {
                rec.programming_mode = on;
                let ok = persist_shared(flash, &rec).await;
                publish_persist(&rec);
                STORAGE_ACK.signal(ok);
            }
            StorageCmd::ReplaceRecord(new_rec) => {
                rec = new_rec;
                if rec.device.id == 0 {
                    ensure_device_id(&mut rec, boot_entropy);
                }
                if rec.wifi_hostname.is_empty() {
                    ensure_wifi_hostname(&mut rec, boot_entropy);
                }
                let ok = persist_shared(flash, &rec).await;
                publish_persist(&rec);
                STORAGE_ACK.signal(ok);
            }
            StorageCmd::Clear => {
                rec = PersistRecord::default();
                ensure_device_id(&mut rec, boot_entropy);
                ensure_wifi_hostname(&mut rec, boot_entropy);
                let _ = persist_shared(flash, &rec).await;
                publish_persist(&rec);
            }
        }
    }
}
