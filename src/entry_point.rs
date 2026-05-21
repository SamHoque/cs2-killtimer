use std::sync::Arc;

use crate::common::APP_NAME;
use crate::core::state::SharedState;

#[cfg(windows)]
fn ensure_console() {
    use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AllocConsole, AttachConsole};
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
            let _ = AllocConsole();
        }
    }
}

pub fn start() -> anyhow::Result<()> {
    #[cfg(windows)]
    ensure_console();

    crate::utils::logger::info(&format!("{APP_NAME}: fetching offsets from cs2-dumper"));
    let off = crate::cs2::offsets::fetch_offsets()?;
    crate::utils::logger::info(&format!(
        "offsets loaded | dwLocalPlayerController=0x{:X} m_pActionTrackingServices=0x{:X} m_iNumRoundKills=0x{:X}",
        off.dw_local_player_controller, off.m_p_action_tracking_services, off.m_i_num_round_kills,
    ));

    let state = Arc::new(SharedState::new());
    let worker_state = state.clone();
    std::thread::spawn(move || crate::hacks::kill_timer::run(worker_state, off));

    crate::utils::render::overlay::run(state)
}
