#!/bin/sh
set -eu
APP="${APP:-$HOME/Applications/Local Transcribe.app}"
BIN="Local Transcribe.app/Contents/MacOS/voice-to-text-desktop"
open -a "$APP"
sleep 2
first=$(pgrep -f "$BIN" | wc -l | tr -d ' ')
open -a "$APP"
rc=$?
sleep 2
second=$(pgrep -f "$BIN" | wc -l | tr -d ' ')
echo "first_launch_processes=$first second_launch_processes=$second second_exit=$rc"
test "$second" = "1"
test "$rc" = "0"
echo "PASS single instance"
