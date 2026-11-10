// A player typed one of the commands registered above (API 8).
fn observe_perform_command(
    game: &mut crate::game::Game<'_>,
    player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::solcarrot_state::on_perform_command(game, player, command, ran);
}
