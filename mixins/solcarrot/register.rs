// World open (API 8): declare `/foodlist size|sync|clear [target]`.
fn observe_register_commands(commands: &mut Vec<NativeCommand>) {
    crate::game::solcarrot_state::on_register_commands(commands);
}
