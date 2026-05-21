//! Kill detection, streak timing, and overlay state publication.

use std::sync::Arc;
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::common::{TARGET_MODULE, TARGET_PROCESS};
use crate::core::settings::{
    OVERLAY_HIDDEN_RGB, OVERLAY_TIMER_RGB_HIGH, OVERLAY_TIMER_RGB_LOW, OVERLAY_TIMER_RGB_MID,
    POLL_COMBAT, POLL_IDLE, POLL_TIMER,
};
use crate::core::state::SharedState;
use crate::cs2::game::{
    active_weapon_id, enemies_alive, find_local_controller, is_alive, is_round_live, round_kills,
    team,
};
use crate::cs2::offsets::Offsets;
use crate::cs2::weapons::{display_name, is_kill_ignored};
use crate::utils::logger;
use crate::utils::memory::Process;

fn timer_rgb(shown: u32) -> (u8, u8, u8) {
    if shown < 10 {
        OVERLAY_TIMER_RGB_LOW
    } else if shown < 15 {
        OVERLAY_TIMER_RGB_MID
    } else {
        OVERLAY_TIMER_RGB_HIGH
    }
}

/// Blocking worker loop. Designed to run on its own thread; returns only on panic.
pub fn run(state: Arc<SharedState>, off: Offsets) -> ! {
    let mut announced_waiting = false;
    loop {
        let proc = match Process::attach(TARGET_PROCESS) {
            Ok(p) => p,
            Err(_) => {
                if !announced_waiting {
                    logger::warn(&format!("waiting for {TARGET_PROCESS}..."));
                    announced_waiting = true;
                }
                state.set(false, 1, OVERLAY_HIDDEN_RGB, false);
                sleep(POLL_IDLE);
                continue;
            }
        };
        let client = match proc.module(TARGET_MODULE) {
            Ok(m) => m,
            Err(_) => {
                if !announced_waiting {
                    logger::warn(&format!("attached, waiting for {TARGET_MODULE}..."));
                    announced_waiting = true;
                }
                state.set(false, 1, OVERLAY_HIDDEN_RGB, false);
                sleep(POLL_IDLE);
                continue;
            }
        };

        logger::info(&format!("attached to {TARGET_PROCESS} (pid={})", proc.pid));
        announced_waiting = false;
        state.set(false, 1, OVERLAY_HIDDEN_RGB, true);

        let mut prev_round_kills: i32 = -1;
        let mut streak_start: Option<Instant> = None;

        loop {
            let poll;

            let entity_list = client.add(off.dw_entity_list).deref();
            if entity_list.is_null() {
                break;
            }

            // dw_local_player_controller is unreliable in current cs2-dumper
            // (stores a handle-shaped value, not a pointer). Find the local
            // controller by matching m_hPawn against the local pawn pointer.
            let pawn = client.add(off.dw_local_player_pawn).deref();
            if pawn.is_null() {
                state.set(false, 1, OVERLAY_HIDDEN_RGB, true);
                streak_start = None;
                prev_round_kills = -1;
                sleep(POLL_IDLE);
                continue;
            }
            let ctrl = find_local_controller(entity_list, pawn, &off);
            if ctrl.is_null() {
                state.set(false, 1, OVERLAY_HIDDEN_RGB, true);
                streak_start = None;
                prev_round_kills = -1;
                sleep(POLL_IDLE);
                continue;
            }

            let alive = is_alive(pawn, &off);
            let live_round = is_round_live(client, &off);
            let local_team = team(pawn, &off);
            let kills = round_kills(ctrl, &off);

            // First-tick baseline.
            if prev_round_kills < 0 {
                prev_round_kills = kills;
                sleep(if alive { POLL_COMBAT } else { POLL_IDLE });
                continue;
            }

            // Dead or round not live -> hide.
            if !alive || !live_round {
                if streak_start.is_some() {
                    let reason = if !alive { "local death" } else { "round ended" };
                    logger::info(&format!("streak stopped ({reason})"));
                }
                streak_start = None;
                state.set(false, 1, OVERLAY_HIDDEN_RGB, true);
                prev_round_kills = kills;
                sleep(if alive { POLL_COMBAT } else { POLL_IDLE });
                continue;
            }

            // New kill(s). Ignored kills (non-firearm / last enemy) leave the
            // existing streak alone — they don't reset or stop it.
            if kills > prev_round_kills {
                let delta = kills - prev_round_kills;
                prev_round_kills = kills;
                let weapon_id = active_weapon_id(entity_list, pawn, &off);
                for _ in 0..delta {
                    let remaining = enemies_alive(entity_list, pawn, local_team, &off);
                    if remaining == 0 || is_kill_ignored(weapon_id) {
                        continue;
                    }
                    let was_running = streak_start.is_some();
                    streak_start = Some(Instant::now());
                    logger::info(&format!(
                        "{} | weapon={} kills={kills} enemies_alive={remaining}",
                        if was_running { "streak reset" } else { "streak started" },
                        display_name(weapon_id)
                    ));
                    state.set(true, 1, timer_rgb(1), true);
                }
            } else if kills < prev_round_kills {
                prev_round_kills = kills;
                if streak_start.is_some() {
                    logger::info("streak stopped (round kills reset)");
                }
                streak_start = None;
                state.set(false, 1, OVERLAY_HIDDEN_RGB, true);
            }

            // Publish current timer state.
            if let Some(start) = streak_start {
                let shown = start.elapsed().as_secs() as u32 + 1;
                state.set(true, shown, timer_rgb(shown), true);
                poll = POLL_TIMER;
            } else {
                state.set(false, 1, OVERLAY_HIDDEN_RGB, true);
                poll = if alive { POLL_COMBAT } else { POLL_IDLE };
            }

            sleep(poll);
        }

        logger::warn(&format!("lost {TARGET_PROCESS}, reattaching..."));
        state.set(false, 1, OVERLAY_HIDDEN_RGB, false);
        sleep(Duration::from_millis(500));
    }
}
