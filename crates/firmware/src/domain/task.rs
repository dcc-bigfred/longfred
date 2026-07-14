//! Task domeny: konsument wejścia i zdarzeń serwera, producent komend WiThrottle.

use embassy_futures::select::{select, Either};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_time::{Duration, Instant, Timer};
use longfred_proto::protocol::Cmd;

use crate::config;
use crate::domain::state::{DomainState, CMD_BUF};
use crate::domain::DOMAIN_STATE;
use crate::input;
use crate::net;

async fn flush_cmds(
    cmd_tx: &embassy_sync::channel::Sender<'static, CriticalSectionRawMutex, Cmd, { net::WIT_COMMANDS_DEPTH }>,
    out: &mut heapless::Vec<Cmd, CMD_BUF>,
    last_cmd: &mut Instant,
) {
    let min_delay = Duration::from_millis(config::network::OUTBOUND_COMMANDS_MIN_DELAY_MS);
    while let Some(cmd) = out.first().cloned() {
        let elapsed = last_cmd.elapsed();
        if elapsed < min_delay {
            Timer::after(min_delay - elapsed).await;
        }
        let _ = out.remove(0);
        cmd_tx.send(cmd).await;
        *last_cmd = Instant::now();
    }
}

#[embassy_executor::task]
pub async fn task() {
    let mut state = DomainState::new();
    let input_rx = input::INPUT_CHANNEL.receiver();
    let events_rx = net::WIT_EVENTS.receiver();
    let cmd_tx = net::WIT_COMMANDS.sender();
    let snap_tx = DOMAIN_STATE.sender();
    let mut out: heapless::Vec<Cmd, CMD_BUF> = heapless::Vec::new();
    let mut last_cmd = Instant::now() - Duration::from_secs(1);

    snap_tx.send(state.snapshot());

    loop {
        out.clear();
        let changed = match select(input_rx.receive(), events_rx.receive()).await {
            Either::First(ev) => state.apply_input(ev, &mut out),
            Either::Second(sev) => state.apply_event(sev, &mut out),
        };
        flush_cmds(&cmd_tx, &mut out, &mut last_cmd).await;
        if changed {
            snap_tx.send(state.snapshot());
        }
    }
}
