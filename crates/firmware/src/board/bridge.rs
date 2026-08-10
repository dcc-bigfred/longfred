//! Raw → ControlSurface → INPUT_CHANNEL bridge task.

use embassy_futures::select::{select, Either};
use embassy_time::{Duration, Instant, Timer};

use crate::board::raw::RAW_CHANNEL;
use crate::board::ControlSurface;
use crate::input::INPUT_CHANNEL;

#[cfg(any(
    feature = "variant-longfred-standard",
    feature = "variant-longfred-mini"
))]
use crate::board::variants::longfred_family::LongFredSurface;
#[cfg(feature = "variant-markwtech")]
use crate::board::variants::markwtech::MarkwtechSurface;
#[cfg(feature = "variant-heiko-wifred")]
use crate::board::variants::heiko_wifred::HeikoWifredSurface;

const TICK_MS: u64 = 50;

#[embassy_executor::task]
pub async fn task() {
    #[cfg(feature = "variant-longfred-mini")]
    let mut surface = LongFredSurface::mini();
    #[cfg(feature = "variant-longfred-standard")]
    let mut surface = LongFredSurface::standard();
    #[cfg(feature = "variant-markwtech")]
    let mut surface = MarkwtechSurface::new();
    #[cfg(feature = "variant-heiko-wifred")]
    let mut surface = HeikoWifredSurface::new();

    let raw_rx = RAW_CHANNEL.receiver();
    let input_tx = INPUT_CHANNEL.sender();

    let desc = surface.descriptor();
    log::info!("board bridge: variant={}", desc.id);

    loop {
        match select(
            raw_rx.receive(),
            Timer::after(Duration::from_millis(TICK_MS)),
        )
        .await
        {
            Either::First(ev) => {
                let now = Instant::now();
                surface.on_raw(ev, now, &mut |ie| {
                    let _ = input_tx.try_send(ie);
                });
            }
            Either::Second(()) => {
                let now = Instant::now();
                surface.tick(now, &mut |ie| {
                    let _ = input_tx.try_send(ie);
                });
            }
        }
    }
}
