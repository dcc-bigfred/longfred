#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
#[cfg(not(feature = "sim"))]
use esp_hal::clock::CpuClock;
use esp_hal::rng::Rng;
use esp_hal::timer::timg::TimerGroup;
use esp_println as _;
use esp_storage::FlashStorage;
use log::info;
use static_cell::StaticCell;

#[cfg(not(feature = "sim"))]
use embassy_net::{Config as NetConfig, DhcpConfig, StackResources};
#[cfg(not(feature = "sim"))]
use esp_radio::wifi::{Interface, WifiController};

use longfred_firmware::{config, domain, input, storage, ui};
#[cfg(not(feature = "sim"))]
use longfred_firmware::power;
#[cfg(not(feature = "sim"))]
use longfred_firmware::net;

esp_bootloader_esp_idf::esp_app_desc!();

static FLASH: StaticCell<FlashStorage> = StaticCell::new();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();

    // Wokwi: default clock is enough and avoids I2C edge-timing quirks at max MHz.
    #[cfg(feature = "sim")]
    let hal_cfg = esp_hal::Config::default();
    #[cfg(not(feature = "sim"))]
    let hal_cfg = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(hal_cfg);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_int =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let rng = Rng::new();
    let boot_entropy = rng.random();

    let flash = FLASH.init(FlashStorage::new(peripherals.FLASH));
    let wifi_hostname = storage::ensure_boot_hostname(flash, boot_entropy);
    info!("wifi hostname: {}", wifi_hostname.as_str());

    #[cfg(not(feature = "sim"))]
    {
        let seed = ((rng.random() as u64) << 32) | rng.random() as u64;
        net::WIFI_HOSTNAME.sender().send(wifi_hostname.clone());

        let controller = WifiController::new(peripherals.WIFI, Default::default())
            .expect("WifiController::new");
        let sta = Interface::station();

        static RESOURCES: StaticCell<StackResources<{ config::sizes::NET_SOCKETS }>> =
            StaticCell::new();
        let resources = RESOURCES.init(StackResources::new());
        let mut dhcp = DhcpConfig::default();
        let mut host = heapless::String::<32>::new();
        let _ = host.push_str(wifi_hostname.as_str());
        dhcp.hostname = Some(host);
        let (stack, runner) = embassy_net::new(sta, NetConfig::dhcpv4(dhcp), resources, seed);

        if let Ok(token) = net::wifi::connection(controller) {
            spawner.spawn(token);
        }
        if let Ok(token) = net::wifi::net_task(runner) {
            spawner.spawn(token);
        }
        if let Ok(token) = net::wifi::status_task(stack) {
            spawner.spawn(token);
        }
        if let Ok(token) = net::wifi::config_task(stack) {
            spawner.spawn(token);
        }
        if let Ok(token) = net::mdns::task(stack, config::network::NETWORKS[0].ssid) {
            spawner.spawn(token);
        }
        if let Ok(token) = net::session::task(stack) {
            spawner.spawn(token);
        }
    }

    #[cfg(feature = "sim")]
    info!("sim: WiFi/net bring-up skipped");

    info!(
        "LongFred boot: {} | throttles={} | networks={}",
        config::DEFAULT_DEVICE_NAME,
        config::buttons::DEFAULT_THROTTLES,
        config::network::NETWORKS.len()
    );

    #[cfg(not(feature = "sim"))]
    {
        if let Ok(token) = storage::task(flash, boot_entropy) {
            spawner.spawn(token);
        }
        if config::power::USE_BATTERY_TEST {
            if let Ok(token) = power::battery::task(peripherals.ADC1, peripherals.GPIO1) {
                spawner.spawn(token);
            }
        }
        if let Ok(token) = power::sleep::task(peripherals.LPWR, peripherals.GPIO0) {
            spawner.spawn(token);
        }
    }
    #[cfg(feature = "sim")]
    {
        let _ = flash;
        info!("sim: storage/battery/sleep skipped");
    }

    #[cfg(feature = "sim_bare")]
    info!("sim_bare: no tasks spawned, heartbeat only");

    #[cfg(not(feature = "sim_bare"))]
    {
        let sender = input::INPUT_CHANNEL.sender();

        info!("main: i2c init");
        let (oled_i2c, expander_i2c) = input::i2c_bus::init(peripherals.I2C0);
        let enc = input::encoder::build();
        let nav = input::gpio_nav::build(
            peripherals.GPIO18,
            peripherals.GPIO19,
            peripherals.GPIO20,
            peripherals.GPIO21,
            peripherals.GPIO22,
            peripherals.GPIO23,
            peripherals.GPIO10,
        );

        // OLED before expander: shared I2C — init display before MCP probe NACKs.
        if let Ok(token) = ui::display::task(oled_i2c) {
            spawner.spawn(token);
        }
        if let Ok(token) = input::gpio_nav::task(nav, sender) {
            spawner.spawn(token);
        }
        if let Ok(token) = input::expander::task(expander_i2c, sender) {
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

        let _ = sender;
    }

    loop {
        Timer::after(Duration::from_secs(60)).await;
    }
}
