//! Raw → ControlSurface → INPUT_CHANNEL bridge task.

use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Instant, Timer};

use crate::board::ControlSurface;
use crate::board::raw::RAW_CHANNEL;
use crate::input::INPUT_CHANNEL;

#[cfg(feature = "variant-longfred-v1")]
use crate::board::variants::longfred_family::LongFredSurface;
#[cfg(feature = "variant-markwtech")]
use crate::board::variants::markwtech::MarkwtechSurface;

const TICK_MS: u64 = 50;

#[embassy_executor::task]
pub async fn task() {
    #[cfg(feature = "variant-longfred-v1")]
    let mut surface = LongFredSurface::v1();
    #[cfg(feature = "variant-markwtech")]
    let mut surface = MarkwtechSurface::new();

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
