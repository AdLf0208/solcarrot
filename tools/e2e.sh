#!/usr/bin/env bash
# In-game check for Spice of Life: Carrot Edition slice 1.
#
# The upstream defaults are milestones 5/10/15/20/25 and +2 hearts per
# milestone, so the first rise is at the FIFTH unique food (20 -> 24). The
# check eats five different foods, proves the fourth does not raise the bar,
# proves re-eating a known food and a sixth new food do not either, then clears
# the list and regrows it for a restart-persistence check.
#
# MODE=control writes `hearts_per_milestone = 0` before the launch: five foods
# must leave max health at 20. That is the differential: an implementation that
# ignored the config would reach 24 and this run fails.
#
# Every use is a real right-click held for 2.2 s (the food takes 1.6 s), and the
# mod's own `/foodlist size` output is cross-checked against `ctl state`'s max
# health.
set -euo pipefail

CLONE=${CAMPAIGN_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}
PORT=$CLONE/ports/solcarrot
CTL=${CTL:-$PORT/target/horizon-cargo/fast/horizon-ctl}
GAME=${GAME:-$PORT/target/horizon-cargo/fast/horizon-client}
RES=${RES:-${HORIZON_RES:-${HORIZON_RES:-$HOME/.local/horizon-client/26.3}}}
SOCK=${SOCK:-/tmp/opencode/solcarrot-e2e.sock}
MODE=${MODE:-default}
WORLD=SolCarrotE2E

[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }

SCRATCH=$(mktemp -d /tmp/opencode/solcarrot-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"
rm -f "$SOCK"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
fail() { echo "FAIL: $*" >&2; exit 1; }
max_health() {
    # `ctl state` has no max-health field at this pin; the vanilla attribute
    # command reads the server body's computed value. Chat is newest first.
    ctl chat "/attribute @s minecraft:max_health get" >/dev/null
    ctl wait 5 >/dev/null
    ctl messages 5 | jq -r '.chat[0]' \
        | grep -oE 'is -?[0-9]+(\.[0-9]+)?' | head -1 | awk '{print $2}' | cut -d. -f1
}
size() {
    ctl chat "/foodlist size" >/dev/null
    ctl wait 5 >/dev/null
    ctl messages 8 | jq -r '.chat[]' | grep -o '[0-9]* unique foods eaten' | head -1 | awk '{print $1}'
}
eat() {
    ctl give "$1" 1
    ctl wait 5
    ctl select "$1"
    ctl use-item --hold 2.2
    ctl wait 6
}
expect_health() {
    local got
    got=$(max_health)
    if [ "$got" != "$1" ]; then
        ctl state | jq -c '.player' >&2 || true
        fail "$2: expected max health $1, got ${got:-nothing}"
    fi
    echo "max health $got ($2)"
}

cleanup() {
    rc=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    rm -rf "$SCRATCH" 2>/dev/null || true
    echo "EXIT:$rc"
}
trap cleanup EXIT

launch() {
    rm -f "$SOCK"
    cd "$SCRATCH"
    "$CTL" --socket "$SOCK" launch \
        --game "$GAME" --world "$WORLD" --seed 20261103 --creative --headless --wait \
        --resources "$RES"
    ctl wait-ready 120
    ctl wait 10
    ctl command "execute in minecraft:the_end run tp @s 0.5 64 0.5"
    ctl wait 5
    ctl command "execute in minecraft:the_end run fill -4 63 -4 4 63 4 minecraft:stone"
    ctl command "execute in minecraft:the_end run tp @s 0.5 64 0.5"
    ctl wait 10
}

if [ "$MODE" = control ]; then
    cat > "$XDG_CONFIG_HOME/solcarrot.conf" <<'EOF'
base_hearts = 10
hearts_per_milestone = 0
milestones = 5,10,15,20,25
blacklist =
whitelist =
minimum_food_value = 1
reset_on_death = false
limit_progression_to_survival = false
EOF
    echo "control: hearts_per_milestone=0"
fi

echo "== launch (creative, The End arena) =="
launch
expect_health 20 "baseline"

echo "== a use released before it finishes is not counted =="
# 0.6 s of a 1.6 s food use: the release is a `use_item_stop`, not a finish, so
# the exact finished-use hook must not record it (the old tick sampling had to
# infer this; the hook makes it impossible to record).
ctl give minecraft:golden_carrot 1
ctl wait 5
ctl select minecraft:golden_carrot
ctl use-item --hold 0.6
ctl wait 6
n=$(size)
[ "$n" = 0 ] || fail "a released-early food was counted ($n)"
expect_health 20 "released early"

echo "== eat apple, bread, carrot, baked potato =="
for food in minecraft:apple minecraft:bread minecraft:carrot minecraft:baked_potato; do
    eat "$food"
done
expect_health 20 "four foods: below the first milestone"
n=$(size)
[ "$n" = 4 ] || fail "expected 4 unique foods before the fifth, got $n"

echo "== fifth food reaches the first milestone =="
eat minecraft:cooked_beef
if [ "$MODE" = control ]; then
    expect_health 20 "control: config turns the bonus off"
else
    expect_health 24 "five foods: first milestone (+2 hearts)"
fi
n=$(size)
[ "$n" = 5 ] || fail "expected 5 unique foods, got $n"

echo "== a known food and a sixth new food do not raise it again =="
eat minecraft:apple
expect_health "$(if [ "$MODE" = control ]; then echo 20; else echo 24; fi)" "re-eating a known food"
eat minecraft:cooked_chicken
expect_health "$(if [ "$MODE" = control ]; then echo 20; else echo 24; fi)" "sixth new food below the next milestone"
n=$(size)
[ "$n" = 6 ] || fail "expected 6 unique foods, got $n"

echo "== a potion is a consumable but not food =="
eat minecraft:potion
n=$(size)
[ "$n" = 6 ] || fail "a potion changed the food count to $n"

echo "== reset command =="
ctl chat "/foodlist clear"
ctl wait 8
expect_health 20 "clear drops the bonus"
n=$(size)
[ "$n" = 0 ] || fail "expected 0 foods after clear, got $n"

if [ "$MODE" = control ]; then
    ctl quit >/dev/null 2>&1 || true
    trap - EXIT
    rm -rf "$SCRATCH"
    echo "CONTROL PASS: five foods, a re-eat and a sixth food stayed at 20 max health with hearts_per_milestone=0"
    echo "EXIT:0"
    exit 0
fi

echo "== regrow five foods, then restart the world =="
for food in minecraft:apple minecraft:bread minecraft:carrot minecraft:baked_potato minecraft:cooked_beef; do
    eat "$food"
done
expect_health 24 "regrown five foods"
store="$SCRATCH/saves/$WORLD/solcarrot/foods.tsv"
[ -s "$store" ] || fail "missing history file $store"
grep -q 'minecraft:apple' "$store" || fail "history file has no apple: $(cat "$store")"
echo "history file: $(wc -l < "$store") lines"
ctl quit >/dev/null 2>&1 || true
sleep 2

echo "== relaunch the same world =="
launch
expect_health 24 "history reloaded after restart"
n=$(size)
[ "$n" = 5 ] || fail "expected 5 foods after restart, got $n"

ctl quit >/dev/null 2>&1 || true
trap - EXIT
rm -rf "$SCRATCH"
echo "PASS: 4 foods stayed 20, 5 reached 24, a re-eat and a sixth stayed 24, a potion was ignored, clear returned to 20, and 24 survived a restart"
echo "EXIT:0"
