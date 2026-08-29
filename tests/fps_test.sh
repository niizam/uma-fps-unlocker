#!/bin/sh
# Deterministic black-box tests for fps.sh. No Proton, no game, no network.
# Runs fps.sh against fake proton/game/injector executables and asserts on
# observable behavior (argv forwarding, option forwarding, exit status, errors).

set -u

REPO=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
FPS_SH="$REPO/fps.sh"

PASS=0
FAIL=0

fail() {
    echo "FAIL: $*" >&2
    FAIL=$((FAIL + 1))
}

ok() {
    PASS=$((PASS + 1))
}

# --- Fixture setup -----------------------------------------------------------
setup() {
    TMP=$(mktemp -d) || exit 1
    cp "$FPS_SH" "$TMP/fps.sh"
    chmod +x "$TMP/fps.sh"

    : > "$TMP/uma_unlocker.dll"

    cat > "$TMP/uma_unlock_jp.exe" <<'EOF'
#!/bin/sh
echo "injector args: $*" >> "$INJECTOR_LOG"
exit "${INJECTOR_EXIT:-0}"
EOF
    chmod +x "$TMP/uma_unlock_jp.exe"

    cat > "$TMP/uma_unlock_global.exe" <<'EOF'
#!/bin/sh
echo "injector args: $*" >> "$INJECTOR_LOG"
exit "${INJECTOR_EXIT:-0}"
EOF
    chmod +x "$TMP/uma_unlock_global.exe"

    cat > "$TMP/proton" <<'EOF'
#!/bin/sh
echo "proton args: $*" >> "$PROTON_LOG"
case "$1" in
    runinprefix)
        shift
        echo "runinprefix args: $*" >> "$PROTON_LOG"
        "$@"
        exit $?
        ;;
    run)
        shift
        "$@"
        exit $?
        ;;
esac
EOF
    chmod +x "$TMP/proton"

    cat > "$TMP/game" <<'EOF'
#!/bin/sh
{
    echo "game argc: $#"
    for a in "$@"; do
        echo "game arg: $a"
    done
} >> "$GAME_LOG"
[ -z "${GAME_DELAY:-}" ] || sleep "$GAME_DELAY"
exit "${GAME_EXIT:-0}"
EOF
    chmod +x "$TMP/game"

    export PROTON_LOG="$TMP/proton.log"
    export INJECTOR_LOG="$TMP/injector.log"
    export GAME_LOG="$TMP/game.log"
    : > "$PROTON_LOG"
    : > "$INJECTOR_LOG"
    : > "$GAME_LOG"
}

teardown() {
    rm -rf "$TMP"
}

# --- Test: exact argv forwarding including a spaced argument -----------------
test_argv_forwarding() {
    setup
    GAME_DELAY=1 "$TMP/fps.sh" "$TMP/proton" run "$TMP/game" "hello world" extra >/dev/null 2>&1
    if grep -q '^game arg: hello world$' "$GAME_LOG" && grep -q '^game arg: extra$' "$GAME_LOG"; then
        ok
    else
        fail "argv forwarding: spaced argument not preserved (log: $(cat "$GAME_LOG"))"
    fi
    teardown
}

# --- Test: selected injector and option forwarding ---------------------------
test_injector_and_options() {
    setup
    GAME_DELAY=1 FPS=144 VSYNC=on SERVER=jp WAIT_SECONDS=30 \
        "$TMP/fps.sh" "$TMP/proton" run "$TMP/game" >/dev/null 2>&1
    if grep -q 'uma_unlock_jp.exe --fps 144 --vsync on --wait 30' "$PROTON_LOG"; then
        ok
    else
        fail "injector/options: expected jp injector with options (log: $(cat "$PROTON_LOG"))"
    fi
    teardown

    setup
    GAME_DELAY=1 FPS=60 VSYNC=off SERVER=global WAIT_SECONDS=10 \
        "$TMP/fps.sh" "$TMP/proton" run "$TMP/game" >/dev/null 2>&1
    if grep -q 'uma_unlock_global.exe --fps 60 --vsync off --wait 10' "$PROTON_LOG"; then
        ok
    else
        fail "injector/options: expected global injector with options (log: $(cat "$PROTON_LOG"))"
    fi
    teardown
}

# --- Test: PROTON_PATH override ----------------------------------------------
test_proton_path() {
    setup
    # A second proton that records that it was used.
    cat > "$TMP/other_proton" <<'EOF'
#!/bin/sh
echo "other proton args: $*" >> "$OTHER_LOG"
case "$1" in
    runinprefix)
        shift
        "$@"
        exit $?
        ;;
esac
EOF
    chmod +x "$TMP/other_proton"
    export OTHER_LOG="$TMP/other.log"
    : > "$OTHER_LOG"

    # No 'proton' basename in argv; PROTON_PATH must be used.
    GAME_DELAY=1 PROTON_PATH="$TMP/other_proton" \
        "$TMP/fps.sh" "$TMP/game" >/dev/null 2>&1
    if grep -q 'runinprefix' "$OTHER_LOG"; then
        ok
    else
        fail "PROTON_PATH: override not used (log: $(cat "$OTHER_LOG"))"
    fi
    teardown
}

# --- Test: missing artifacts -------------------------------------------------
test_missing_dll() {
    setup
    rm -f "$TMP/uma_unlocker.dll"
    "$TMP/fps.sh" "$TMP/proton" run "$TMP/game" >/dev/null 2>"$TMP/err"
    status=$?
    if [ "$status" -ne 0 ] && grep -q 'uma_unlocker.dll' "$TMP/err"; then
        ok
    else
        fail "missing dll: expected nonzero exit and dll error (stderr: $(cat "$TMP/err"))"
    fi
    teardown
}

test_missing_injector() {
    setup
    rm -f "$TMP/uma_unlock_global.exe"
    "$TMP/fps.sh" "$TMP/proton" run "$TMP/game" >/dev/null 2>"$TMP/err"
    status=$?
    if [ "$status" -ne 0 ] && grep -q 'uma_unlock_global.exe' "$TMP/err"; then
        ok
    else
        fail "missing injector: expected nonzero exit and injector error (stderr: $(cat "$TMP/err"))"
    fi
    teardown
}

test_missing_command() {
    setup
    "$TMP/fps.sh" >/dev/null 2>"$TMP/err"
    status=$?
    if [ "$status" -ne 0 ] && grep -q 'missing Steam command' "$TMP/err"; then
        ok
    else
        fail "missing command: expected nonzero exit and launch-options error (stderr: $(cat "$TMP/err"))"
    fi
    teardown
}

# --- Test: invalid config ----------------------------------------------------
test_invalid_config() {
    for bad in \
        "FPS=abc" \
        "FPS=0" \
        "VSYNC=maybe" \
        "SERVER=eu" \
        "WAIT_SECONDS=abc" \
        "WAIT_SECONDS=0"; do
        setup
        env $bad "$TMP/fps.sh" "$TMP/proton" run "$TMP/game" >/dev/null 2>"$TMP/err"
        status=$?
        if [ "$status" -ne 0 ]; then
            ok
        else
            fail "invalid config '$bad': expected nonzero exit"
        fi
        teardown
    done
}

# --- Test: game exit status is returned --------------------------------------
test_game_exit_status() {
    setup
    GAME_EXIT=42 "$TMP/fps.sh" "$TMP/proton" run "$TMP/game" >/dev/null 2>&1
    status=$?
    if [ "$status" -eq 42 ]; then
        ok
    else
        fail "game exit status: expected 42, got $status"
    fi
    teardown
}

# --- Test: injection failure does not terminate the game ---------------------
test_injection_failure_keeps_game() {
    setup
    # Injector fails, but the game still runs and its status is returned.
    GAME_DELAY=1 INJECTOR_EXIT=7 GAME_EXIT=3 "$TMP/fps.sh" "$TMP/proton" run "$TMP/game" >/dev/null 2>"$TMP/err"
    if [ $? -eq 3 ] && grep -q 'injection failed' "$TMP/err"; then
        ok
    else
        fail "injection failure: expected game status 3 and injection-failed message (stderr: $(cat "$TMP/err"))"
    fi
    teardown
}

test_argv_forwarding
test_injector_and_options
test_proton_path
test_missing_dll
test_missing_injector
test_missing_command
test_invalid_config
test_game_exit_status
test_injection_failure_keeps_game

echo "fps.sh tests: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
