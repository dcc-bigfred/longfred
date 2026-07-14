#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_net::{Config as NetConfig, StackResources};
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::rng::Rng;
use esp_hal::timer::timg::TimerGroup;
use esp_println as _;
use esp_radio::wifi::{Interface, WifiController};
use log::info;
use static_cell::StaticCell;

use longfred_firmware::{config, domain, input, net, ui};

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();

    let hal_cfg = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(hal_cfg);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_int =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    // Sterta wymagana przez esp-radio (bloby WiFi).
    esp_alloc::heap_allocator!(size: 72 * 1024);

    // --- WiFi STA + stos sieciowy ---
    let rng = Rng::new();
    let seed = ((rng.random() as u64) << 32) | rng.random() as u64;

    let controller = WifiController::new(peripherals.WIFI, Default::default())
        .expect("WifiController::new");
    let sta = Interface::station();

    static RESOURCES: StaticCell<StackResources<{ config::sizes::NET_SOCKETS }>> = StaticCell::new();
    let resources = RESOURCES.init(StackResources::new());
    let (stack, runner) = embassy_net::new(
        sta,
        NetConfig::dhcpv4(Default::default()),
        resources,
        seed,
    );

    info!(
        "LongFred boot: {} | throttles={} | networks={}",
        config::DEVICE_NAME,
        config::buttons::DEFAULT_THROTTLES,
        config::network::NETWORKS.len()
    );

    if let Ok(token) = net::wifi::connection(controller) {
        spawner.spawn(token);
    }
    if let Ok(token) = net::wifi::net_task(runner) {
        spawner.spawn(token);
    }
    if let Ok(token) = net::wifi::status_task(stack) {
        spawner.spawn(token);
    }
    if let Ok(token) = net::mdns::task(stack, config::network::NETWORKS[0].ssid) {
        spawner.spawn(token);
    }
    if let Ok(token) = net::wit::task(stack) {
        spawner.spawn(token);
    }

    let sender = input::INPUT_CHANNEL.sender();

    let (rows, cols) = input::keypad::build();
    let enc = input::encoder::build();

    if let Ok(token) = input::keypad::task(rows, cols, sender) {
        spawner.spawn(token);
    }
    if let Ok(token) = input::encoder::task(enc.a, enc.b, sender) {
        spawner.spawn(token);
    }
    if let Ok(token) = input::encoder::button_task(enc.button, sender) {
        spawner.spawn(token);
    }
    if let Ok(token) = domain::task::task() {
        spawner.spawn(token);
    }

    let oled_i2c = ui::display::build_i2c(peripherals.I2C0);
    if let Ok(token) = ui::display::task(oled_i2c) {
        spawner.spawn(token);
    }

    loop {
        Timer::after(Duration::from_secs(5)).await;
        info!("main alive");
    }
}
