#!/bin/bash
set -e

# Setup Paths
WORKSPACE_ROOT=$(pwd)
SBC_RS_PATH="$WORKSPACE_ROOT/sbc-rs"
TEMPLATE_PATH="$WORKSPACE_ROOT/../sing-box-config-templates/config.template.json"
OUTPUT_PATH="$WORKSPACE_ROOT/tests/config.gen.json"

echo "=== CI Validation Started ==="
echo "Building sbc-rs..."
cd "$SBC_RS_PATH"
cargo build --quiet
SBC_BIN="$SBC_RS_PATH/target/debug/sbc-rs"
cd "$WORKSPACE_ROOT"

if [ ! -f "$TEMPLATE_PATH" ]; then
    echo "Error: Template not found at $TEMPLATE_PATH"
    exit 1
fi

echo "Generating Mock Environment..."
# Export Mock Variables
export CLASH_API_SECRET="test_secret"
export MIXED_PROXY_USERNAME="admin"
export MIXED_PROXY_PASSWORD="123"
export PROVIDER_NAME_1="Provider1"
export PROVIDER_NAME_2="Provider2"
export PROVIDER_NAME_3="Provider3"
export SUB_URL_1="http://example.com/1"
export SUB_URL_2="http://example.com/2"
export SUB_URL_3="http://example.com/3"

# JSON Injection Mocks (Empty lists/objects for simplicity, or specific test values)
export DNS_SERVERS='[{"tag":"injected_dns","address":"1.1.1.1","detour":"DIRECT"}]'
export DNS_RULES_MID='{"rule_set":"geosite-category-ads-all","server":"local","action":"reject"}'
export DNS_RULES_BOTTOM='{"server":"local"}'
export ROUTE_RULES_TOP='{"protocol":"dns","action":"hijack-dns"}'
export ROUTE_RULES_MID='{"ip_cidr":["1.0.0.1/32"],"outbound":"DIRECT"}'
export ROUTE_RULES_BOTTOM='{"port":80,"outbound":"DIRECT"}'
export ROUTE_RULE_SETS='{"tag":"test-rule-set","type":"local","format":"source","path":"/tmp/test.json"}'
export INBOUNDS_TOP='{"type":"mixed","tag":"mixed-in","listen":"::","listen_port":2080}'
export INBOUNDS_BOTTOM='{"type":"direct","tag":"dns-in-2","network":"udp"}'
export EXPERIMENTAL_CLASH_API="" # Should be ignored/empty
export EXPERIMENTAL_CACHE_FILE="" # Should be ignored/empty

# 5. Test 'render' command
echo "Running sbc-rs render..."
"$SBC_BIN" render --template "$TEMPLATE_PATH" --output "$OUTPUT_PATH"

# 6. Test 'update' command (Mock Server)
echo "Running sbc-rs update (Mock Server)..."
mkdir -p /tmp/www
echo '{"inbounds": []}' > /tmp/www/template.json # Minimal valid config signature
echo "MOCK_ENV=1" > /tmp/www/env.example

# Start background mock server
cd /tmp/www
python3 -m http.server 8080 &
SERVER_PID=$!
cd -

sleep 1

"$SBC_BIN" update \
    --template-url "http://localhost:8080/template.json" \
    --template-path "$TEMPLATE_PATH.new" \
    --env-url "http://localhost:8080/env.example" \
    --env-path "$MOCK_ENV_PATH.new" || { kill $SERVER_PID; exit 1; }

kill $SERVER_PID

if grep -q "inbounds" "$TEMPLATE_PATH.new"; then
    echo "✅ Update command validation passed."
else
    echo "❌ Update command validation failed."
    exit 1
fi

# 7. Test Daemon Mode (Run/Stop)
echo "Testing Daemon Mode..."
# Create mock sing-box binary
cat << 'EOF' > /tmp/sing-box
#!/bin/bash
if [ "$1" == "run" ]; then
    echo "Mock sing-box running..."
    # Simulate long running process handling signals
    trap 'echo "Mock received TERM"; exit 0' SIGTERM
    while true; do sleep 1; done
fi
EOF
chmod +x /tmp/sing-box
export PATH="/tmp:$PATH" # Put mock in path

# Override command in sbc-rs? No, sbc-rs calls "sing-box". We rely on PATH.
# Link mock to expected location if hardcoded?
# sbc-rs uses "sing-box" in Command::new(). PATH is sufficient.

echo "Starting sbc-rs run (background)..."
# Override PID file location via env var (feature added for testing)
export SBC_PID_FILE="/tmp/sing-box.pid"

"$SBC_BIN" run --config "$OUTPUT_PATH" > /tmp/daemon.log 2>&1 &
DAEMON_PID=$!
sleep 2

if grep -q "Started sing-box supervisor" /tmp/daemon.log || grep -q "sing-box started with PID" /tmp/daemon.log; then
    echo "✅ Daemon started."
else
    echo "❌ Daemon failed to start."
    cat /tmp/daemon.log
    kill $DAEMON_PID
    exit 1
fi

echo "Stopping daemon..."
# Stop also needs the same env var
export SBC_PID_FILE="/tmp/sing-box.pid"
"$SBC_BIN" stop

sleep 1
if kill -0 $DAEMON_PID 2>/dev/null; then
    echo "❌ Daemon did not exit."
    kill $DAEMON_PID
    exit 1
else
    echo "✅ Daemon exited gracefully."
fi

# Clean up
rm /tmp/sing-box

echo "Validating Output with sing-box check..."
if command -v sing-box &> /dev/null; then
    # sing-box check needs the rulesets referenced in config to exist? 
    # Usually check -c verifies syntax. If it needs ext resources, it might fail?
    # verify syntax only?
    # sing-box check loads config.
    sing-box check -c "$OUTPUT_PATH" || { echo "sing-box check failed!"; exit 1; }
    echo "sing-box check PASSED."
else
    echo "Warning: sing-box binary not found, skipping syntax check. (JSON structure is valid if sbc-rs succeeded)"
fi

echo "=== CI Validation PASSED ==="
rm -f "$OUTPUT_PATH"
