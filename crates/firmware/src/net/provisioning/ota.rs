//! Dual-slot OTA via `esp-bootloader-esp-idf` `OtaUpdater`.

use embedded_storage::nor_flash::NorFlash;
use embassy_net::tcp::TcpSocket;
use esp_bootloader_esp_idf::ota::OtaImageState;
use esp_bootloader_esp_idf::ota_updater::OtaUpdater;
use esp_bootloader_esp_idf::partitions::FlashRegion;
use esp_storage::FlashStorage;
use log::{info, warn};
use longfred_proto::image::{
    ESP_IMAGE_HEADER_LEN, ImageError, validate_esp32c6_app_image,
};

const SECTOR: usize = 4096;
const PT_BUF: usize = 3072;

/// Mark the running slot Valid after a successful OTA boot (no-op if otadata is empty).
pub fn mark_running_slot_valid(flash: &mut FlashStorage<'_>) {
    let mut buf = [0u8; PT_BUF];
    let Ok(mut ota) = OtaUpdater::new(flash, &mut buf) else {
        return;
    };
    match ota.current_ota_state() {
        Ok(OtaImageState::New | OtaImageState::PendingVerify) => {
            if ota.set_current_ota_state(OtaImageState::Valid).is_ok() {
                info!("ota: running slot marked Valid");
            }
        }
        Err(e) => warn!("ota: current state: {:?}", e),
        Ok(_) => {}
    }
}

fn image_err_msg(e: ImageError) -> &'static str {
    match e {
        ImageError::Truncated => "image header truncated",
        ImageError::BadMagic => "not an ESP app image (use .app.bin, not merged)",
        ImageError::WrongChip => "image is not for ESP32-C6",
        ImageError::TooSmall => "image too small",
        ImageError::TooLarge => "image larger than OTA slot",
    }
}

fn write_sector(
    region: &mut FlashRegion<'_, FlashStorage<'_>>,
    offset: &mut u32,
    sector: &mut [u8; SECTOR],
    filled: &mut usize,
    chunk: &[u8],
) -> Result<(), &'static str> {
    let mut rest = chunk;
    while !rest.is_empty() {
        let room = SECTOR - *filled;
        let n = rest.len().min(room);
        sector[*filled..*filled + n].copy_from_slice(&rest[..n]);
        *filled += n;
        rest = &rest[n..];
        if *filled == SECTOR {
            region.write(*offset, sector).map_err(|_| "flash write")?;
            *offset += SECTOR as u32;
            *filled = 0;
            *sector = [0xFFu8; SECTOR];
        }
    }
    Ok(())
}

/// Stream `content_len` bytes from `sock` into the inactive OTA slot.
pub async fn flash_from_socket(
    flash: &mut FlashStorage<'_>,
    sock: &mut TcpSocket<'_>,
    already: &[u8],
    content_len: usize,
) -> Result<(), &'static str> {
    let mut buf = [0u8; PT_BUF];
    let mut ota = OtaUpdater::new(flash, &mut buf).map_err(|_| "ota partitions missing")?;

    let mut header = [0u8; ESP_IMAGE_HEADER_LEN];
    let mut header_got = already.len().min(ESP_IMAGE_HEADER_LEN);
    if header_got > 0 {
        header[..header_got].copy_from_slice(&already[..header_got]);
    }
    while header_got < ESP_IMAGE_HEADER_LEN && header_got < content_len {
        let end = ESP_IMAGE_HEADER_LEN.min(content_len);
        match sock.read(&mut header[header_got..end]).await {
            Ok(0) => return Err("eof body"),
            Ok(k) => header_got += k,
            Err(_) => return Err("read body"),
        }
    }

    let slot_len;
    {
        let (region, subtype) = ota.next_partition().map_err(|_| "no next ota slot")?;
        info!("ota: writing slot {:?}", subtype);
        slot_len = region.partition_size();
    }

    if already.len() > content_len {
        return Err("bad body framing");
    }
    validate_esp32c6_app_image(
        &header[..header_got.min(ESP_IMAGE_HEADER_LEN)],
        content_len,
        slot_len,
    )
    .map_err(image_err_msg)?;

    let mut offset: u32 = 0;
    let mut sector = [0xFFu8; SECTOR];
    let mut filled = 0usize;
    let mut written = 0usize;

    {
        let (mut region, _) = ota.next_partition().map_err(|_| "no next ota slot")?;
        write_sector(
            &mut region,
            &mut offset,
            &mut sector,
            &mut filled,
            &header[..header_got],
        )?;
        written += header_got;
        if already.len() > header_got {
            write_sector(
                &mut region,
                &mut offset,
                &mut sector,
                &mut filled,
                &already[header_got..],
            )?;
            written += already.len() - header_got;
        }

        let mut tmp = [0u8; 1024];
        while written < content_len {
            let want = (content_len - written).min(tmp.len());
            match sock.read(&mut tmp[..want]).await {
                Ok(0) => return Err("eof body"),
                Ok(k) => {
                    write_sector(&mut region, &mut offset, &mut sector, &mut filled, &tmp[..k])?;
                    written += k;
                }
                Err(_) => return Err("read body"),
            }
        }
        if filled > 0 {
            let padded = filled.div_ceil(4) * 4;
            region
                .write(offset, &sector[..padded])
                .map_err(|_| "flash write")?;
        }
    }

    ota.activate_next_partition()
        .map_err(|_| "activate ota slot")?;
    let _ = ota.set_current_ota_state(OtaImageState::New);
    info!("ota: slot activated ({written} bytes)");
    Ok(())
}
