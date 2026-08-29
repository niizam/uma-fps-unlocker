#!/bin/sh
# fps.sh - Linux Steam Proton launcher for uma-fps-unlocker.
#
# Steam launch options:
#   sh ./fps.sh %command%
#
# This wrapper launches the game through the original Proton command and then
# injects uma_unlocker.dll into the running game via the same prefix using
# Proton's `runinprefix` verb. No proxy DLL and no LD_PRELOAD are used.
#
# Configuration is read from the environment (see README for details):
#   FPS          positive integer, default 120
#   VSYNC        on|off, default off
#   SERVER       jp|global, default global
#   WAIT_SECONDS positive integer, default 60
#   PROTON_PATH  optional absolute path to the Proton script (overrides argv scan)

die() {
    echo "fps.sh: $*" >&2
    exit 1
}

is_positive_int() {
    case "$1" in
        ''|*[!0-9]*) return 1 ;;
    esac
    [ "$1" -gt 0 ]
}

# Physical directory of this script, independent of the current working directory.
SCRIPT_DIR=$(CDPATH= cd "$(dirname "$0")" && pwd) || die "could not resolve script directory"

# --- Configuration -----------------------------------------------------------
FPS="${FPS:-120}"
VSYNC="${VSYNC:-off}"
SERVER="${SERVER:-global}"
WAIT_SECONDS="${WAIT_SECONDS:-60}"

if ! is_positive_int "$FPS"; then
    die "FPS must be a positive integer (got '$FPS')"
fi
case "$VSYNC" in
    on|off) ;;
    *) die "VSYNC must be 'on' or 'off' (got '$VSYNC')" ;;
esac
case "$SERVER" in
    jp|global) ;;
    *) die "SERVER must be 'jp' or 'global' (got '$SERVER')" ;;
esac
if ! is_positive_int "$WAIT_SECONDS"; then
    die "WAIT_SECONDS must be a positive integer (got '$WAIT_SECONDS')"
fi

case "$SERVER" in
    jp) INJECTOR="uma_unlock_jp.exe" ;;
    global) INJECTOR="uma_unlock_global.exe" ;;
esac

# --- Artifacts ---------------------------------------------------------------
[ -f "$SCRIPT_DIR/uma_unlocker.dll" ] || die "uma_unlocker.dll not found next to fps.sh (expected at $SCRIPT_DIR/uma_unlocker.dll)"
[ -f "$SCRIPT_DIR/$INJECTOR" ] || die "$INJECTOR not found next to fps.sh (expected at $SCRIPT_DIR/$INJECTOR)"

# --- Proton discovery --------------------------------------------------------
[ "$#" -gt 0 ] || die "missing Steam command; use launch options: sh ./fps.sh %command%"

if [ -n "$PROTON_PATH" ]; then
    PROTON="$PROTON_PATH"
else
    PROTON=""
    for arg in "$@"; do
        if [ "${arg##*/}" = "proton" ]; then
            PROTON="$arg"
            break
        fi
    done
    [ -n "$PROTON" ] || die "could not find the Proton script in the command arguments; set PROTON_PATH to its absolute path"
fi
[ -x "$PROTON" ] || die "Proton script is not executable: $PROTON"

# --- Launch ------------------------------------------------------------------
GAME_PID=""
INJECT_PID=""

forward_signal() {
    signal=$1
    [ -z "$INJECT_PID" ] || kill -"$signal" "$INJECT_PID" 2>/dev/null
    [ -z "$GAME_PID" ] || kill -"$signal" "$GAME_PID" 2>/dev/null
}

trap 'forward_signal TERM' TERM
trap 'forward_signal INT' INT

# Start the original full command (the game) in the background and keep its PID.
"$@" &
GAME_PID=$!

# Inject through the same prefix. STEAM_COMPAT_* environment is inherited.
(
    proton_pid=""
    stop_injector() {
        [ -z "$proton_pid" ] || kill -TERM "$proton_pid" 2>/dev/null
        [ -z "$proton_pid" ] || wait "$proton_pid" 2>/dev/null
        exit 143
    }
    trap 'stop_injector' TERM INT

    "$PROTON" runinprefix "$SCRIPT_DIR/$INJECTOR" --fps "$FPS" --vsync "$VSYNC" --wait "$WAIT_SECONDS" &
    proton_pid=$!
    wait "$proton_pid"
    status=$?
    if [ "$status" -ne 0 ]; then
        echo "fps.sh: injection failed (status $status)." >&2
    fi
    exit "$status"
) &
INJECT_PID=$!

# Reap the game and return its exit status.
wait "$GAME_PID"
GAME_STATUS=$?

if kill -0 "$INJECT_PID" 2>/dev/null; then
    kill -TERM "$INJECT_PID" 2>/dev/null
fi
wait "$INJECT_PID" 2>/dev/null

exit "$GAME_STATUS"
