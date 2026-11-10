// A food's use ran its time (API 90, `living_entity_use_item_finish`): the
// point where upstream's `FoodList` records the food. `stack` is the stack the
// acting player just finished; the glue checks it is a food.
fn observe_use_finish(game: &mut crate::game::Game<'_>, stack: &ItemStack) {
    crate::game::solcarrot_state::on_use_item_finish(game, stack);
}
