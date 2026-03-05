//! Equalizer commands.

#[tauri::command]
pub fn get_eq_gains() -> Vec<f32> {
    crate::player::eq::get_all_gains().to_vec()
}

#[tauri::command]
pub fn set_eq_gains(gains: Vec<f32>) {
    crate::player::eq::set_all_gains(&gains);
}

#[tauri::command]
pub fn get_eq_enabled() -> bool {
    crate::player::eq::is_enabled()
}

#[tauri::command]
pub fn set_eq_enabled(enabled: bool) {
    crate::player::eq::set_enabled(enabled);
}
