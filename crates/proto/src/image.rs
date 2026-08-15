//! ESP-IDF application image header checks (host-testable).

/// ESP-IDF app image magic (`esp_image_header_t.magic`).
pub const ESP_IMAGE_MAGIC: u8 = 0xE9;

/// ESP32-C6 chip id in the app image header (`chip_id`, little-endian).
pub const ESP32C6_CHIP_ID: u16 = 0x000D;

/// Size of `esp_image_header_t`.
pub const ESP_IMAGE_HEADER_LEN: usize = 24;

/// Offset of `chip_id` in the app image header.
const CHIP_ID_OFF: usize = 12;

/// Why an uploaded image was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageError {
    /// Fewer than [`ESP_IMAGE_HEADER_LEN`] bytes.
    Truncated,
    /// Magic byte is not [`ESP_IMAGE_MAGIC`].
    BadMagic,
    /// `chip_id` is not ESP32-C6.
    WrongChip,
    /// Declared length is smaller than the header.
    TooSmall,
    /// Declared length exceeds the inactive OTA slot.
    TooLarge,
}

/// Validate an ESP32-C6 app image (not a merged flash dump).
pub fn validate_esp32c6_app_image(
    prefix: &[u8],
    content_len: usize,
    slot_len: usize,
) -> Result<(), ImageError> {
    if content_len < ESP_IMAGE_HEADER_LEN {
        return Err(ImageError::TooSmall);
    }
    if content_len > slot_len {
        return Err(ImageError::TooLarge);
    }
    if prefix.len() < ESP_IMAGE_HEADER_LEN {
        return Err(ImageError::Truncated);
    }
    if prefix[0] != ESP_IMAGE_MAGIC {
        return Err(ImageError::BadMagic);
    }
    let chip = u16::from_le_bytes([prefix[CHIP_ID_OFF], prefix[CHIP_ID_OFF + 1]]);
    if chip != ESP32C6_CHIP_ID {
        return Err(ImageError::WrongChip);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(magic: u8, chip: u16) -> [u8; ESP_IMAGE_HEADER_LEN] {
        let mut h = [0u8; ESP_IMAGE_HEADER_LEN];
        h[0] = magic;
        let b = chip.to_le_bytes();
        h[CHIP_ID_OFF] = b[0];
        h[CHIP_ID_OFF + 1] = b[1];
        h
    }

    #[test]
    fn accepts_c6_app_header() {
        let h = header(ESP_IMAGE_MAGIC, ESP32C6_CHIP_ID);
        assert_eq!(validate_esp32c6_app_image(&h, 4096, 0x3C0000), Ok(()));
    }

    #[test]
    fn rejects_wrong_magic() {
        let h = header(0x00, ESP32C6_CHIP_ID);
        assert_eq!(
            validate_esp32c6_app_image(&h, 4096, 0x3C0000),
            Err(ImageError::BadMagic)
        );
    }

    #[test]
    fn rejects_wrong_chip() {
        let h = header(ESP_IMAGE_MAGIC, 0x0005);
        assert_eq!(
            validate_esp32c6_app_image(&h, 4096, 0x3C0000),
            Err(ImageError::WrongChip)
        );
    }

    #[test]
    fn rejects_oversized() {
        let h = header(ESP_IMAGE_MAGIC, ESP32C6_CHIP_ID);
        assert_eq!(
            validate_esp32c6_app_image(&h, 0x3C0001, 0x3C0000),
            Err(ImageError::TooLarge)
        );
    }

    #[test]
    fn rejects_truncated_prefix() {
        let h = header(ESP_IMAGE_MAGIC, ESP32C6_CHIP_ID);
        assert_eq!(
            validate_esp32c6_app_image(&h[..8], 4096, 0x3C0000),
            Err(ImageError::Truncated)
        );
    }
}
