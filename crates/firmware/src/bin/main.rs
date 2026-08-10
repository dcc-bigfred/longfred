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

#[cfg(not(feature = "sim"))]
use longfred_firmware::net;
#[cfg(not(feature = "sim"))]
use longfred_firmware::power;
use longfred_firmware::{board, config, domain, input, storage, ui};

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
    let boot = storage::ensure_boot(flash, boot_entropy);
    info!("wifi hostname: {}", boot.wifi_hostname.as_str());

    let enter_programming = boot.programming_mode
        || (board::active_variant().auto_pair_when_unconfigured && !boot.has_wifi_credentials);

    #[cfg(not(feature = "sim"))]
    {
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
            if !boot.programming_mode {
                storage::write_record(flash, &prog_rec);
            }

            let _ = net::provisioning::spawn_programming_net(
                &spawner,
                peripherals.WIFI,
                seed,
                prog_rec,
            );
        } else {
            let controller = match WifiController::new(peripherals.WIFI, Default::default()) {
                Ok(c) => c,
                Err(e) => {
                    log::error!("boot: WifiController::new failed: {:?} — hanging", e);
                    loop {
                        Timer::after(Duration::from_secs(60)).await;
                    }
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
    }

    #[cfg(feature = "sim")]
    {
        let _ = enter_programming;
        info!("sim: WiFi/net bring-up skipped");
    }

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
        let raw_sender = board::RAW_CHANNEL.sender();
        info!("board variant: {}", board::active().id);

        info!("main: i2c init");
        let (oled_i2c, expander_i2c) = input::i2c_bus::init(peripherals.I2C0);

        // OLED for variants with a display; heiko uses LED presenter instead.
        #[cfg(not(feature = "variant-heiko-wifred"))]
        if let Ok(token) = ui::display::task(oled_i2c) {
            spawner.spawn(token);
        }
        #[cfg(feature = "variant-heiko-wifred")]
        {
            let _ = oled_i2c;
            let (led_stop, led_fwd, led_rev) = ui::led_presenter::build();
            if let Ok(token) = ui::led_presenter::task(led_stop, led_fwd, led_rev) {
                spawner.spawn(token);
            }
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
            if let Ok(token) = input::gpio_nav::task(nav, raw_sender) {
                spawner.spawn(token);
            }
        }

        // MarkWTech: 3×4 keypad matrix (pins from markwtech constants).
        #[cfg(feature = "variant-markwtech")]
        {
            let keypad = input::keypad::build();
            if let Ok(token) = input::keypad::task(keypad, raw_sender) {
                spawner.spawn(token);
            }
        }

        // Expanders: LongFred family + heiko-wifred.
        #[cfg(any(
            feature = "variant-longfred-standard",
            feature = "variant-longfred-mini",
            feature = "variant-heiko-wifred"
        ))]
        if let Ok(token) = input::expander::task(expander_i2c, raw_sender) {
            spawner.spawn(token);
        }
        #[cfg(feature = "variant-markwtech")]
        {
            let _ = expander_i2c;
        }

        // Encoder: LongFred family + markwtech (heiko uses pot).
        #[cfg(not(feature = "variant-heiko-wifred"))]
        {
            let enc = input::encoder::build();
            if let Ok(token) = input::encoder::task(enc.a, enc.b, raw_sender) {
                spawner.spawn(token);
            }
            if let Ok(token) = input::encoder::button_task(enc.button, raw_sender) {
                spawner.spawn(token);
            }
        }

        if let Ok(token) = board::bridge::task() {
            spawner.spawn(token);
        }

        // Normal throttle domain only when not in Soft-AP programming path.
        #[cfg(feature = "sim")]
        let spawn_domain = true;
        #[cfg(not(feature = "sim"))]
        let spawn_domain = !enter_programming;
        if spawn_domain {
            if let Ok(token) = domain::task::task() {
                spawner.spawn(token);
            }
        }

        let _ = raw_sender;
    }

    loop {
        Timer::after(Duration::from_secs(60)).await;
    }
}
