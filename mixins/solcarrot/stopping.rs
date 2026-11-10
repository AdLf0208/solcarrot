// The server is stopping (API 44): flush the history file.
fn observe_server_stopping(game: &mut crate::game::Game<'_>) {
    crate::game::solcarrot_state::on_server_stopping(game);
}
