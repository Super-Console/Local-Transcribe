#!/bin/sh
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CLI="$ROOT/target/debug/local-transcribe"
NODE="$ROOT/target/debug/local-transcribe-node"
SERVICE="$ROOT/target/debug/voice-to-text-service"
APP_DATA="$HOME/Library/Application Support/com.timmy.voicetotextapp"
MODEL="$APP_DATA/models/sherpa-onnx-fire-red-asr2-zh_en-int8-2026-02-26"
WORK="/tmp/lte2e-$$"
mkdir -p "$WORK/data" "$WORK/node"
cp "$APP_DATA/selected-model.json" "$WORK/data/selected-model.json"
printf '%s' '{"mode":"local_only","nodes":[{"id":"loop","name":"loop","host":"127.0.0.1","port":7899,"model_id":null,"enabled":true}]}' > "$WORK/data/lan-settings.json"
ffmpeg -loglevel error -f lavfi -i anullsrc=r=16000:cl=mono -t 3600 -c:a pcm_s16le "$WORK/long.wav"
ffmpeg -loglevel error -f lavfi -i anullsrc=r=16000:cl=mono -t 300 -c:a pcm_s16le "$WORK/cancel.wav"
nohup "$NODE" --name loop --data-dir "$WORK/node" --listen 127.0.0.1:7899 --compute on --model-dir "$MODEL" --idle-unload 120 > "$WORK/node.log" 2>&1 &
NODE_PID=$!
nohup "$SERVICE" serve --data-dir "$WORK/data" --allow-root "$WORK" --allow-root "/private$WORK" > "$WORK/service.log" 2>&1 &
SERVICE_PID=$!
trap 'kill "$NODE_PID" "$SERVICE_PID" 2>/dev/null || true' EXIT
for i in $(seq 1 40); do [ -S "$WORK/data/service.sock" ] && break; sleep 0.25; done
sleep 2
curl -sf -m 3 http://127.0.0.1:7899/api/v1/health >/dev/null

submit() { "$CLI" jobs add "${MEDIA:-$WORK/long.wav}" --root "$WORK" --data-dir "$WORK/data" "$@" | python3 -c 'import json,sys;print(json.load(sys.stdin)["job"]["jobId"])'; }
list_json() { "$CLI" jobs list --data-dir "$WORK/data"; }

LOCAL_ID=$(submit)
REMOTE_ID=$(submit --node loop)
JSON=$(list_json)
ACTIVE=$(printf '%s' "$JSON" | python3 -c 'import json,sys;d=json.load(sys.stdin);print(d.get("active") or "")')
PENDING=$(printf '%s' "$JSON" | python3 -c 'import json,sys;d=json.load(sys.stdin);print(len(d.get("pending") or []))')
echo "serial active=$ACTIVE pending=$PENDING remote=$REMOTE_ID local=$LOCAL_ID"
test -n "$ACTIVE"
test "$PENDING" -ge 1
ACTIVE_ID="$ACTIVE"
"$CLI" jobs cancel "$ACTIVE_ID" --data-dir "$WORK/data" >/dev/null || true
"$CLI" jobs remove "$LOCAL_ID" --data-dir "$WORK/data" >/dev/null || true
i=0
while [ $i -lt 60 ]; do
  STATE=$("$CLI" jobs show "$ACTIVE_ID" --data-dir "$WORK/data" | python3 -c 'import json,sys;print(json.load(sys.stdin)["job"]["state"])')
  [ "$STATE" = "cancelled" ] && break
  sleep 0.5
  i=$((i+1))
done
echo "serial cleanup active_state=$STATE"

CANCEL_ID=$(MEDIA="$WORK/cancel.wav" submit --node loop)
sleep 3
"$CLI" jobs cancel "$CANCEL_ID" --data-dir "$WORK/data" >/dev/null
i=0
while [ $i -lt 60 ]; do
  STATE=$("$CLI" jobs show "$CANCEL_ID" --data-dir "$WORK/data" | python3 -c 'import json,sys;print(json.load(sys.stdin)["job"]["state"])')
  [ "$STATE" = "cancelled" ] && break
  sleep 0.5
  i=$((i+1))
done
NODE_STATE=$(curl -sf -m 3 http://127.0.0.1:7899/api/v1/tasks | python3 -c 'import json,sys;ts=json.load(sys.stdin);print(next((t["state"] for t in reversed(ts) if t["file_name"]=="long.wav"),"none"))')
echo "cancel local=$STATE node=$NODE_STATE"
test "$STATE" = "cancelled"
case "$NODE_STATE" in cancelled|cancel_requested) ;; *) exit 1;; esac
echo "PASS lan behaviour (serial + cancel)"
