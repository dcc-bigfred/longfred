//! Persistence (NVS): WiFi passwords, saved locos, device identity (one NVS sector).

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
use esp_bootloader_esp_idf::partitions::{
    read_partition_table, DataPartitionSubType, PartitionType,
};
use esp_hal::rng::Rng;
use esp_storage::FlashStorage;
use heapless::String;
use log::{info, warn};
use longfred_proto::persist::{
    id_from_entropy, wifi_hostname_from_entropy, DeviceIdentity, Language, PersistRecord, SavedLoco,
    StaticIpConfig, MAX_SAVED_LOCOS, MAX_WIFI_HOSTNAME_LEN,
};

pub static PERSIST_LOADED: Signal<CriticalSectionRawMutex, PersistRecord> = Signal::new();

/// Signalled after a storage write that requested acknowledgement
/// ([`StorageCmd::SetProgrammingMode`], [`StorageCmd::ReplaceRecord`]).
pub static STORAGE_ACK: Signal<CriticalSectionRawMutex, ()> = Signal::new();

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
    SetProgrammingMode(bool),
    ReplaceRecord(PersistRecord),
    Clear,
}

pub static STORAGE_CTRL: Channel<CriticalSectionRawMutex, StorageCmd, 4> = Channel::new();

/// Boot-time NVS snapshot used to choose STA vs programming path.
#[derive(Clone)]
pub struct BootState {
    pub wifi_hostname: heapless::String<MAX_WIFI_HOSTNAME_LEN>,
    pub programming_mode: bool,
    pub has_wifi_credentials: bool,
    pub record: PersistRecord,
}

const SECTOR: usize = 4096;
const PT_BUF_LEN: usize = 4096;

fn load(flash: &mut FlashStorage<'_>) -> Option<PersistRecord> {
    let mut pt_buf = [0u8; PT_BUF_LEN];
    let pt = read_partition_table(flash, &mut pt_buf).ok()?;
    let nvs = pt
        .find_partition(PartitionType::Data(DataPartitionSubType::Nvs))
        .ok()??;
    let mut region = nvs.as_embedded_storage(flash);
    let mut sector = [0u8; SECTOR];
    ReadNorFlash::read(&mut region, 0, &mut sector).ok()?;
    PersistRecord::decode(&sector)
}

fn persist(flash: &mut FlashStorage<'_>, rec: &PersistRecord) {
    let mut pt_buf = [0u8; PT_BUF_LEN];
    let Ok(pt) = read_partition_table(flash, &mut pt_buf) else {
        warn!("storage: partition table read failed");
        return;
    };
    let Ok(Some(nvs)) = pt.find_partition(PartitionType::Data(DataPartitionSubType::Nvs)) else {
        warn!("storage: nvs partition not found");
        return;
    };
    let mut region = nvs.as_embedded_storage(flash);
    let mut sector = [0xFFu8; SECTOR];
    let Some(n) = rec.encode(&mut sector) else {
        warn!("storage: encode failed");
        return;
    };
    if region.erase(0, SECTOR as u32).is_err() {
        warn!("storage: erase failed");
        return;
    }
    if region.write(0, &sector[..n]).is_err() {
        warn!("storage: write failed");
    }
}

/// Synchronous NVS write (boot path before the storage task runs).
pub fn write_record(flash: &mut FlashStorage<'_>, rec: &PersistRecord) {
    persist(flash, rec);
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
    if dirty {
        persist(flash, &rec);
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
pub async fn task(flash: &'static mut FlashStorage<'static>, boot_entropy: u32) {
    let mut rec = load(flash).unwrap_or_default();
    let mut dirty = false;
    if rec.device.id == 0 {
        ensure_device_id(&mut rec, boot_entropy);
        dirty = true;
    }
    if rec.wifi_hostname.is_empty() {
        ensure_wifi_hostname(&mut rec, boot_entropy);
        dirty = true;
    }
    if dirty {
        persist(flash, &rec);
    }
    info!("wifi hostname: {}", rec.wifi_hostname.as_str());
    PERSIST_LOADED.signal(rec.clone());

    let rx = STORAGE_CTRL.receiver();
    loop {
        match rx.receive().await {
            StorageCmd::SavePassword { ssid, password } => {
                rec.set_password(ssid.as_str(), password.as_str());
                persist(flash, &rec);
                PERSIST_LOADED.signal(rec.clone());
            }
            StorageCmd::SaveLocos(locos) => {
                rec.locos = locos;
                persist(flash, &rec);
                PERSIST_LOADED.signal(rec.clone());
            }
            StorageCmd::SaveNetwork(cfg) => {
                rec.network = Some(cfg);
                persist(flash, &rec);
                PERSIST_LOADED.signal(rec.clone());
            }
            StorageCmd::SaveDevice(device) => {
                rec.device = device;
                persist(flash, &rec);
                PERSIST_LOADED.signal(rec.clone());
            }
            StorageCmd::RegenerateDeviceId => {
                regenerate_device_id(&mut rec);
                persist(flash, &rec);
                PERSIST_LOADED.signal(rec.clone());
            }
            StorageCmd::SaveLanguage(lang) => {
                rec.language = lang;
                persist(flash, &rec);
                PERSIST_LOADED.signal(rec.clone());
            }
            StorageCmd::SetProgrammingMode(on) => {
                rec.programming_mode = on;
                persist(flash, &rec);
                PERSIST_LOADED.signal(rec.clone());
                STORAGE_ACK.signal(());
            }
            StorageCmd::ReplaceRecord(new_rec) => {
                rec = new_rec;
                if rec.device.id == 0 {
                    ensure_device_id(&mut rec, boot_entropy);
                }
                if rec.wifi_hostname.is_empty() {
                    ensure_wifi_hostname(&mut rec, boot_entropy);
                }
                persist(flash, &rec);
                PERSIST_LOADED.signal(rec.clone());
                STORAGE_ACK.signal(());
            }
            StorageCmd::Clear => {
                rec = PersistRecord::default();
                ensure_device_id(&mut rec, boot_entropy);
                ensure_wifi_hostname(&mut rec, boot_entropy);
                persist(flash, &rec);
                PERSIST_LOADED.signal(rec.clone());
            }
        }
    }
}
