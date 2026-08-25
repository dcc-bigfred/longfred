#![no_std]
#![no_main]
//! LongFred firmware entry point: HAL init, task spawn, and Soft-AP programming mode.

use embassy_executor::Spawner;
use embassy_net::{Config as NetConfig, DhcpConfig, StackResources};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::rng::Rng;
use esp_hal::timer::timg::TimerGroup;
use esp_println as _;
use esp_radio::wifi::{Interface, WifiController};
use esp_storage::FlashStorage;
use log::info;
use static_cell::StaticCell;

use longfred_firmware::net;
use longfred_firmware::power;
use longfred_firmware::{board, config, domain, input, storage, ui};

esp_bootloader_esp_idf::esp_app_desc!();

static FLASH: StaticCell<Mutex<CriticalSectionRawMutex, FlashStorage<'static>>> = StaticCell::new();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();

    let hal_cfg = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(hal_cfg);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_int =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let rng = Rng::new();
    let boot_entropy = rng.random();

    let mut flash_dev = FlashStorage::new(peripherals.FLASH);
    let boot = storage::ensure_boot(&mut flash_dev, boot_entropy);
    info!("wifi hostname: {}", boot.wifi_hostname.as_str());
    net::provisioning::ota::mark_running_slot_valid(&mut flash_dev);

    let enter_programming = boot.programming_mode
        || (board::active_variant().auto_pair_when_unconfigured && !boot.has_wifi_credentials);

    let mut prog_rec = boot.record.clone();
    if enter_programming {
        prog_rec.programming_mode = true;
        if !boot.programming_mode {
            storage::write_record(&mut flash_dev, &prog_rec);
        }
    }
    let flash = FLASH.init(Mutex::new(flash_dev));

    let seed = ((rng.random() as u64) << 32) | rng.random() as u64;
    net::WIFI_HOSTNAME.sender().send(boot.wifi_hostname.clone());

    if enter_programming {
        info!(
            "boot: programming mode (flag={} auto_pair={} creds={})",
            boot.programming_mode,
            board::active_variant().auto_pair_when_unconfigured,
            boot.has_wifi_credentials
        );
        let mut prog_rec = boot.record.clone();
        prog_rec.programming_mode = true;
        if !net::provisioning::spawn_programming_net(
            &spawner,
            peripherals.WIFI,
            seed,
            prog_rec,
            flash,
        ) {
            log::error!("boot: programming net failed — reset");
            esp_hal::system::software_reset();
        }
    } else {
        let controller = match WifiController::new(peripherals.WIFI, Default::default()) {
            Ok(c) => c,
            Err(e) => {
                log::error!("boot: WifiController::new failed: {:?}", e);
                esp_hal::system::software_reset();
            }
        };
        let sta = Interface::station();

        static RESOURCES: StaticCell<StackResources<{ config::sizes::NET_SOCKETS }>> =
            StaticCell::new();
        let resources = RESOURCES.init(StackResources::new());
        let mut dhcp = DhcpConfig::default();
        let mut host = heapless::String::<32>::new();
        let _ = host.push_str(boot.wifi_hostname.as_str());
        dhcp.hostname = Some(host);
        let (stack, runner) = embassy_net::new(sta, NetConfig::dhcpv4(dhcp), resources, seed);

        longfred_firmware::spawn_or_reset!(spawner, net::wifi::connection(controller), "wifi");
        longfred_firmware::spawn_or_reset!(spawner, net::wifi::net_task(runner), "net");
        longfred_firmware::spawn_or_reset!(spawner, net::wifi::status_task(stack), "net-status");
        longfred_firmware::spawn_or_reset!(spawner, net::wifi::config_task(stack), "net-config");
        longfred_firmware::spawn_or_reset!(spawner, net::mdns::task(stack, ""), "mdns");
        longfred_firmware::spawn_or_reset!(spawner, net::session::task(stack), "session");
        longfred_firmware::spawn_or_reset!(spawner, net::pairing_http::task(stack), "pairing-http");
        longfred_firmware::spawn_or_reset!(spawner, net::ping::task(stack), "ping");
        net::provisioning::spawn_sta_http(&spawner, stack, boot.record.clone(), flash);
    }

    info!(
        "LongFred boot: {} | throttles={} | networks={}",
        config::DEFAULT_DEVICE_NAME,
        config::buttons::DEFAULT_THROTTLES,
        config::network::NETWORKS.len()
    );

    longfred_firmware::spawn_or_reset!(spawner, storage::task(flash, boot_entropy), "storage");
    if config::power::USE_BATTERY_TEST {
        #[cfg(not(feature = "variant-markwtech-v1-1"))]
        longfred_firmware::spawn_or_reset!(
            spawner,
            power::battery::task(peripherals.ADC1, peripherals.GPIO1),
            "battery"
        );
        #[cfg(feature = "variant-markwtech-v1-1")]
        longfred_firmware::spawn_or_reset!(
            spawner,
            power::battery::task(peripherals.ADC1, peripherals.GPIO4, peripherals.GPIO10),
            "battery"
        );
    }
    longfred_firmware::spawn_or_reset!(
        spawner,
        power::sleep::task(peripherals.LPWR, peripherals.GPIO0),
        "sleep"
    );

    let raw_sender = board::RAW_CHANNEL.sender();
    info!("board variant: {}", board::active().id);

    info!("main: i2c init");
    let (oled_i2c, expander_i2c) = input::i2c_bus::init(peripherals.I2C0);

    // OLED for variants with a display; heiko uses LED presenter instead.
    #[cfg(not(feature = "variant-heiko-wifred"))]
    longfred_firmware::spawn_or_reset!(spawner, ui::display::task(oled_i2c), "display");
    #[cfg(feature = "variant-heiko-wifred")]
    {
        let _ = oled_i2c;
        let (led_stop, led_fwd, led_rev) = ui::led_presenter::build();
        longfred_firmware::spawn_or_reset!(
            spawner,
            ui::led_presenter::task(led_stop, led_fwd, led_rev),
            "leds"
        );
    }

    // LongFred family: GPIO nav cluster.
    #[cfg(any(
        feature = "variant-longfred-standard",
        feature = "variant-longfred-mini"
    ))]
    {
        let nav = input::gpio_nav::build(
            peripherals.GPIO18,
            peripherals.GPIO19,
            peripherals.GPIO20,
            peripherals.GPIO21,
            peripherals.GPIO22,
            peripherals.GPIO23,
            peripherals.GPIO10,
        );
        longfred_firmware::spawn_or_reset!(
            spawner,
            input::gpio_nav::task(nav, raw_sender),
            "gpio-nav"
        );
    }

    // MarkWTech: 3×4 keypad matrix + extra tact cluster (pins from markwtech constants).
    #[cfg(feature = "variant-markwtech")]
    {
        let keypad = input::keypad::build();
        longfred_firmware::spawn_or_reset!(
            spawner,
            input::keypad::task(keypad, raw_sender),
            "keypad"
        );
        let extras = input::extra_buttons::build();
        longfred_firmware::spawn_or_reset!(
            spawner,
            input::extra_buttons::task(extras, raw_sender),
            "extra-buttons"
        );
    }

    // Expanders: LongFred family + heiko-wifred.
    #[cfg(any(
        feature = "variant-longfred-standard",
        feature = "variant-longfred-mini",
        feature = "variant-heiko-wifred"
    ))]
    longfred_firmware::spawn_or_reset!(
        spawner,
        input::expander::task(expander_i2c, raw_sender),
        "expander"
    );
    #[cfg(feature = "variant-markwtech")]
    {
        let _ = expander_i2c;
    }

    // Encoder: LongFred family + markwtech (heiko uses pot).
    #[cfg(not(feature = "variant-heiko-wifred"))]
    {
        let enc = input::encoder::build();
        longfred_firmware::spawn_or_reset!(
            spawner,
            input::encoder::task(enc.a, enc.b, raw_sender),
            "encoder"
        );
        longfred_firmware::spawn_or_reset!(
            spawner,
            input::encoder::button_task(enc.button, raw_sender),
            "encoder-btn"
        );
    }

    longfred_firmware::spawn_or_reset!(spawner, board::bridge::task(), "bridge");

    if !enter_programming {
        longfred_firmware::spawn_or_reset!(spawner, domain::task::task(), "domain");
        longfred_firmware::spawn_or_reset!(spawner, domain::task::watchdog_task(), "domain-wdt");
    }

    let _ = raw_sender;

    loop {
        Timer::after(Duration::from_secs(60)).await;
    }
}
