//! Joystick, tact switches, SPDT direction — MCP23017 x2 (I2C polling).
//! Emits [`RawEvent`] for the board ControlSurface bridge.

use embassy_time::{Duration, Timer};
use embedded_hal::i2c::I2c;

use super::i2c_bus::SharedI2cDevice;
use crate::board::raw::{ButtonId, RawEvent, RawSender, SwitchId};
use crate::config::board::{BUTTON_MAP, LogicalButton, MCP_ADDRESSES};

const POLL_MS: u64 = 10;
const DEBOUNCE_TICKS: u8 = 2;

// MCP23017 register addresses
const REG_IODIRA: u8 = 0x00;
const REG_GPPUA: u8 = 0x0C;
const REG_GPIOA: u8 = 0x12;

struct McpState {
    addr: u8,
    stable_a: u8,
    stable_b: u8,
    raw_a: u8,
    raw_b: u8,
    debounce_a: u8,
    debounce_b: u8,
    pressed_a: u8,
    pressed_b: u8,
}

impl McpState {
    fn new(addr: u8) -> Self {
        Self {
            addr,
            stable_a: 0xFF,
            stable_b: 0xFF,
            raw_a: 0xFF,
            raw_b: 0xFF,
            debounce_a: 0,
            debounce_b: 0,
            pressed_a: 0,
            pressed_b: 0,
        }
    }
}

fn mcp_init<I: I2c>(i2c: &mut I, addr: u8) -> Result<(), I::Error> {
    // All pins input with pull-ups.
    i2c.write(addr, &[REG_IODIRA, 0xFF, 0xFF])?;
    i2c.write(addr, &[REG_GPPUA, 0xFF, 0xFF])?;
    Ok(())
}

fn mcp_read<I: I2c>(i2c: &mut I, addr: u8) -> Result<(u8, u8), I::Error> {
    let mut buf = [0u8; 2];
    i2c.write_read(addr, &[REG_GPIOA], &mut buf)?;
    Ok((buf[0], buf[1]))
}

fn pressed_bit(stable: u8, bit: u8) -> bool {
    (stable & (1 << bit)) == 0
}

fn update_debounce(
    raw: u8,
    stable: &mut u8,
    debounce: &mut u8,
    _prev_pressed: &mut u8,
) -> (u8, u8) {
    if raw == *stable {
        *debounce = 0;
        return (0, 0);
    }
    *debounce = debounce.saturating_add(1);
    if *debounce < DEBOUNCE_TICKS {
        return (0, 0);
    }
    let old = *stable;
    *stable = raw;
    *debounce = 0;
    let rising = old & !raw;
    let falling = !old & raw;
    (rising, falling)
}

fn logical_to_button(btn: LogicalButton) -> Option<ButtonId> {
    Some(match btn {
        LogicalButton::JoyUp => ButtonId::JoyUp,
        LogicalButton::JoyDown => ButtonId::JoyDown,
        LogicalButton::JoyLeft => ButtonId::JoyLeft,
        LogicalButton::JoyRight => ButtonId::JoyRight,
        LogicalButton::JoyOk => ButtonId::JoyMenu,
        LogicalButton::Back | LogicalButton::EStop => ButtonId::Stop,
        LogicalButton::Menu => ButtonId::Menu,
        LogicalButton::Direction => return None,
        LogicalButton::F0 => ButtonId::F0,
        LogicalButton::F1 => ButtonId::F1,
        LogicalButton::F2 => ButtonId::F2,
        LogicalButton::F3 => ButtonId::F3,
        LogicalButton::F4 => ButtonId::F4,
        LogicalButton::F5 => ButtonId::F5,
        LogicalButton::F6 => ButtonId::F6,
        LogicalButton::F7 => ButtonId::F7,
        LogicalButton::F8 => ButtonId::F8,
        LogicalButton::F9 => ButtonId::Extra(9),
        LogicalButton::F10 => ButtonId::Extra(10),
    })
}

fn emit_button(btn: LogicalButton, rising: u8, falling: u8, bit: u8, sender: &RawSender) {
    let Some(id) = logical_to_button(btn) else {
        return;
    };
    let mask = 1u8 << bit;
    if rising & mask != 0 {
        let _ = sender.try_send(RawEvent::Button(id, true));
    }
    if falling & mask != 0 {
        let _ = sender.try_send(RawEvent::Button(id, false));
    }
}

fn process_chip(
    mcp: &mut McpState,
    rising_a: u8,
    falling_a: u8,
    rising_b: u8,
    falling_b: u8,
    sender: &RawSender,
) {
    for &(addr, port_a, bit, btn) in BUTTON_MAP.iter() {
        if addr != mcp.addr {
            continue;
        }
        let Some(btn) = btn else { continue };
        if port_a {
            emit_button(btn, rising_a, falling_a, bit, sender);
        } else {
            emit_button(btn, rising_b, falling_b, bit, sender);
        }
    }
}

fn direction_value(stable_a: u8) -> u8 {
    // GPA3 on MCP #1: LOW = Forward (COM to GND), HIGH = Reverse.
    if pressed_bit(stable_a, 3) { 1 } else { 0 }
}

#[embassy_executor::task]
pub async fn task(mut i2c: SharedI2cDevice, sender: RawSender) {
    let mut chips = [
        McpState::new(MCP_ADDRESSES[0]),
        McpState::new(MCP_ADDRESSES[1]),
    ];

    let mut present = 0u8;
    for chip in chips.iter() {
        // SoftwareTimeout on the shared bus bounds missing-slave NACKs.
        if mcp_init(&mut i2c, chip.addr).is_ok() {
            present += 1;
        }
    }
    if present == 0 {
        // No expanders: do not poll — NACKs starve OLED on the shared bus.
        log::warn!("expander: no MCP23017 on I2C, input via encoder only");
        return;
    }
    log::info!("expander: {present} MCP23017 ready");

    // Initial direction sync.
    if let Ok((a, _)) = mcp_read(&mut i2c, MCP_ADDRESSES[1]) {
        chips[1].stable_a = a;
        let _ = sender.try_send(RawEvent::Switch(SwitchId::Direction, direction_value(a)));
    }

    let mut dir_stable = chips[1].stable_a;

    loop {
        for mcp in chips.iter_mut() {
            let Ok((raw_a, raw_b)) = mcp_read(&mut i2c, mcp.addr) else {
                continue;
            };
            mcp.raw_a = raw_a;
            mcp.raw_b = raw_b;

            let (rise_a, fall_a) = update_debounce(
                raw_a,
                &mut mcp.stable_a,
                &mut mcp.debounce_a,
                &mut mcp.pressed_a,
            );
            let (rise_b, fall_b) = update_debounce(
                raw_b,
                &mut mcp.stable_b,
                &mut mcp.debounce_b,
                &mut mcp.pressed_b,
            );

            process_chip(mcp, rise_a, fall_a, rise_b, fall_b, &sender);

            if mcp.addr == MCP_ADDRESSES[1] && mcp.stable_a != dir_stable {
                dir_stable = mcp.stable_a;
                let _ = sender.try_send(RawEvent::Switch(
                    SwitchId::Direction,
                    direction_value(dir_stable),
                ));
            }
        }
        Timer::after(Duration::from_millis(POLL_MS)).await;
    }
}
